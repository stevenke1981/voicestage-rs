#![allow(unsafe_op_in_unsafe_fn)]
use std::{
    collections::HashMap,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::Instant,
};
use voicestage::{
    model::{Character, Draft, EMOTIONS, Project, Segment, atomic_write, new_id},
    service::{self, Event, LlmConfig, SpeechConfig},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    Media::Audio::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
use zeroize::Zeroizing;

const LANG: i32 = 100;
const NEW: i32 = 101;
const OPEN: i32 = 102;
const SAVE: i32 = 103;
const IMPORT: i32 = 104;
const EXPORT: i32 = 105;
const PROJECT_PATH: i32 = 106;
const TITLE: i32 = 107;
const LIST: i32 = 110;
const ADD: i32 = 111;
const DELETE: i32 = 112;
const UP: i32 = 113;
const DOWN: i32 = 114;
const SPEAKER: i32 = 120;
const NAME: i32 = 121;
const VOICE: i32 = 122;
const SPOKEN_LANG: i32 = 123;
const EMOTION: i32 = 124;
const SPEED: i32 = 125;
const ROLE_APPLY: i32 = 126;
const ROLE_ADD: i32 = 127;
const TEXT: i32 = 128;
const APPLY: i32 = 129;
const MODE: i32 = 130;
const KEY: i32 = 131;
const OUTPUT: i32 = 132;
const ONE: i32 = 133;
const BATCH: i32 = 134;
const CANCEL: i32 = 135;
const PLAY: i32 = 136;
const STOP: i32 = 137;
const PRICE: i32 = 138;
const COST: i32 = 139;
const URL: i32 = 140;
const MODEL: i32 = 141;
const LLM_KEY: i32 = 142;
const PROMPT: i32 = 143;
const DRAFT: i32 = 144;
const PREVIEW: i32 = 145;
const APPEND: i32 = 146;
const STATUS: i32 = 150;

// Display order matches the existing mode indices and model::EMOTIONS values.
const MODE_LABELS: [[&str; 3]; 3] = [
    [
        "Mock / 本機測試音",
        "Mock / local test tone",
        "Mock / ローカルテスト音",
    ],
    ["s2.1-pro / 付費", "s2.1-pro / paid", "s2.1-pro / 有料"],
    [
        "s2.1-pro-free / 免費政策",
        "s2.1-pro-free / free policy",
        "s2.1-pro-free / 無料枠の方針",
    ],
];
const EMOTION_LABELS: [[&str; 3]; 7] = [
    ["自然", "Neutral", "自然"],
    ["開心", "Happy", "喜び"],
    ["悲傷", "Sad", "悲しみ"],
    ["生氣", "Angry", "怒り"],
    ["興奮", "Excited", "興奮"],
    ["低語", "Whispering", "ささやき"],
    ["平靜", "Calm", "穏やか"],
];

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct App {
    hwnd: HWND,
    font: HFONT,
    controls: HashMap<i32, HWND>,
    labels: Vec<(HWND, [&'static str; 3])>,
    lang: usize,
    project: Project,
    selected: usize,
    dirty: bool,
    loading: bool,
    rx: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    audio: HashMap<String, PathBuf>,
    last_run: Option<PathBuf>,
    candidate: Option<Draft>,
    test_dir: Option<PathBuf>,
    test_step: usize,
    test_started: Instant,
    test_ticks: u64,
    test_max_gap: u128,
    last_tick: Instant,
    layout: Vec<(HWND, i32, i32, i32, i32)>,
    scroll_x: i32,
    scroll_y: i32,
}
impl App {
    fn tr<'a>(&self, a: [&'a str; 3]) -> &'a str {
        a[self.lang]
    }
    unsafe fn get(&self, id: i32) -> String {
        let h = self.controls[&id];
        let n = GetWindowTextLengthW(h);
        let mut b = vec![0u16; n as usize + 1];
        let len = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
        String::from_utf16_lossy(&b[..len as usize])
    }
    unsafe fn set(&self, id: i32, s: &str) {
        SetWindowTextW(self.controls[&id], w(s).as_ptr());
    }
    unsafe fn combo_index(&self, id: i32) -> usize {
        SendMessageW(self.controls[&id], CB_GETCURSEL, 0, 0).max(0) as usize
    }
    unsafe fn combo_set(&self, id: i32, index: usize) {
        SendMessageW(self.controls[&id], CB_SETCURSEL, index, 0);
    }
    unsafe fn combo_items(&self, id: i32, items: &[&str]) {
        let h = self.controls[&id];
        SendMessageW(h, CB_RESETCONTENT, 0, 0);
        for item in items {
            SendMessageW(h, CB_ADDSTRING, 0, w(item).as_ptr() as isize);
        }
        SendMessageW(h, CB_SETCURSEL, 0, 0);
    }
    unsafe fn localize_choices(&self) {
        for (id, labels) in [
            (MODE, MODE_LABELS.as_slice()),
            (EMOTION, EMOTION_LABELS.as_slice()),
        ] {
            let selected = self.combo_index(id);
            let items: Vec<_> = labels.iter().map(|t| t[self.lang]).collect();
            self.combo_items(id, &items);
            self.combo_set(id, selected);
        }
    }
    unsafe fn update_draft_state(&self) {
        if self.candidate.is_none() {
            self.set(
                PREVIEW,
                self.tr([
                    "尚無草稿。輸入提示並產生草稿，檢查後再附加至劇本。",
                    "No draft yet. Enter a prompt, generate a draft, then review before appending.",
                    "下書きはまだありません。指示を入力して生成し、確認後に台詞へ追加します。",
                ]),
            );
        }
        EnableWindow(
            self.controls[&APPEND],
            (self.candidate.is_some() && self.rx.is_none()) as i32,
        );
    }
    unsafe fn status(&self, s: &str) {
        self.set(STATUS, s);
    }
    unsafe fn confirm(&self, s: &str) -> bool {
        if self.test_dir.is_some() {
            return true;
        }
        MessageBoxW(
            self.hwnd,
            w(s).as_ptr(),
            w("VoiceStage").as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
    unsafe fn error(&self, s: &str) {
        self.status(s);
        if self.test_dir.is_none() {
            MessageBoxW(
                self.hwnd,
                w(s).as_ptr(),
                w("VoiceStage").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[allow(clippy::too_many_arguments)] // Mirrors Win32 control creation and logical bounds.
    unsafe fn control(
        &mut self,
        id: i32,
        class: &str,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> HWND {
        let scale = GetDpiForWindow(self.hwnd) as f64 / 96.0;
        let sc = |v: i32| (v as f64 * scale).round() as i32;
        let h = CreateWindowExW(
            0,
            w(class).as_ptr(),
            w(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            sc(x),
            sc(y),
            sc(width),
            sc(height),
            self.hwnd,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        SendMessageW(h, WM_SETFONT, self.font as usize, 1);
        self.controls.insert(id, h);
        self.layout.push((h, x, y, width, height));
        h
    }
    unsafe fn label(&mut self, id: i32, t: [&'static str; 3], x: i32, y: i32, width: i32) {
        let h = self.control(id, "STATIC", t[self.lang], 0, x, y, width, 21);
        self.labels.push((h, t));
    }
    unsafe fn button(&mut self, id: i32, t: [&'static str; 3], x: i32, y: i32, width: i32) {
        let h = self.control(
            id,
            "BUTTON",
            t[self.lang],
            WS_TABSTOP | BS_PUSHBUTTON as u32 | BS_NOTIFY as u32,
            x,
            y,
            width,
            29,
        );
        self.labels.push((h, t));
    }
    #[allow(clippy::too_many_arguments)] // Form construction: identifier, value, bounds and styles.
    unsafe fn edit(
        &mut self,
        id: i32,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        multi: bool,
        password: bool,
    ) {
        let style = WS_TABSTOP
            | WS_BORDER
            | if multi {
                ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32 | WS_VSCROLL
            } else {
                ES_AUTOHSCROLL as u32
            }
            | if password { ES_PASSWORD as u32 } else { 0 };
        let h = self.control(id, "EDIT", text, style, x, y, width, height);
        SendMessageW(h, EM_SETLIMITTEXT, if multi { 1000000 } else { 2000 }, 0);
    }
    unsafe fn combo(&mut self, id: i32, items: &[&str], x: i32, y: i32, width: i32) {
        self.control(
            id,
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
            x,
            y,
            width,
            230,
        );
        self.combo_items(id, items);
    }
    unsafe fn build(&mut self) {
        self.label(
            201,
            [
                "對話劇本工作室",
                "Dialogue script studio",
                "対話シナリオスタジオ",
            ],
            16,
            15,
            300,
        );
        self.combo(LANG, &["繁體中文", "English", "日本語"], 884, 11, 180);
        self.combo_set(LANG, self.lang);
        self.button(NEW, ["新範本", "New template", "テンプレート"], 16, 43, 130);
        self.button(
            OPEN,
            ["開啟 JSON", "Open JSON", "JSON を開く"],
            154,
            43,
            120,
        );
        self.button(SAVE, ["儲存 JSON", "Save JSON", "JSON 保存"], 282, 43, 120);
        self.button(IMPORT, ["附加 TXT", "Append TXT", "TXT 追加"], 410, 43, 120);
        self.button(EXPORT, ["匯出 TXT", "Export TXT", "TXT 出力"], 538, 43, 120);
        self.label(
            202,
            [
                "專案路徑（TXT 使用同名 .txt）",
                "Project path (TXT uses same name .txt)",
                "プロジェクトパス（TXT は同名 .txt）",
            ],
            16,
            81,
            390,
        );
        self.edit(
            PROJECT_PATH,
            "voicestage-project.json",
            410,
            78,
            654,
            25,
            false,
            false,
        );
        self.label(203, ["標題", "Title", "タイトル"], 16, 113, 70);
        self.edit(TITLE, "VoiceStage", 88, 109, 570, 26, false, false);
        self.label(
            204,
            [
                "01 劇本 — 編輯角色與台詞後套用",
                "01 Script — apply character and line edits",
                "01 台本 — 人物と台詞の編集を適用",
            ],
            16,
            145,
            660,
        );
        self.control(
            LIST,
            "LISTBOX",
            "",
            WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,
            16,
            172,
            280,
            335,
        );
        self.button(ADD, ["加台詞", "Add line", "台詞追加"], 16, 514, 86);
        self.button(DELETE, ["刪除", "Delete", "削除"], 108, 514, 80);
        self.button(UP, ["↑", "↑", "↑"], 194, 514, 46);
        self.button(DOWN, ["↓", "↓", "↓"], 246, 514, 50);
        self.label(205, ["說話角色", "Speaker", "話者"], 314, 174, 100);
        self.combo(SPEAKER, &[], 426, 170, 232);
        self.button(
            ROLE_ADD,
            ["新增角色", "New character", "登場人物追加"],
            672,
            170,
            130,
        );
        self.label(206, ["角色名稱", "Character name", "名前"], 314, 210, 105);
        self.edit(NAME, "", 426, 206, 232, 25, false, false);
        self.label(207, ["聲音 ID", "Voice ID", "音声 ID"], 674, 210, 100);
        self.edit(VOICE, "", 778, 206, 286, 25, false, false);
        self.label(
            208,
            ["語言提示¹", "Language hint¹", "言語ヒント¹"],
            314,
            246,
            105,
        );
        self.combo(SPOKEN_LANG, &["zh-TW", "en", "ja", "auto"], 426, 242, 120);
        self.label(209, ["情緒", "Emotion", "感情"], 562, 246, 80);
        self.combo(EMOTION, EMOTIONS, 640, 242, 130);
        self.label(210, ["語速", "Speed", "速度"], 784, 246, 70);
        self.edit(SPEED, "1.0", 856, 242, 70, 25, false, false);
        self.button(
            ROLE_APPLY,
            ["套用角色", "Apply role", "人物を適用"],
            936,
            240,
            128,
        );
        self.label(
            211,
            [
                "¹ 編輯提示；Fish 語音語言由文字自動偵測。",
                "¹ Editorial hint; Fish detects speech language from text.",
                "¹ 編集用ヒント。音声言語はテキストから自動検出。",
            ],
            314,
            279,
            750,
        );
        self.edit(TEXT, "", 314, 307, 750, 149, true, false);
        self.button(
            APPLY,
            ["套用台詞", "Apply line", "台詞を適用"],
            314,
            468,
            130,
        );
        self.button(
            PLAY,
            ["預覽選取音訊", "Play selected", "選択音声を再生"],
            458,
            468,
            155,
        );
        self.button(
            STOP,
            ["停止播放", "Stop playback", "再生停止"],
            625,
            468,
            135,
        );
        self.label(
            212,
            [
                "Mock 是合成測試音，不是 Fish 語音。",
                "Mock is a synthetic test tone, not Fish speech.",
                "Mock は合成テスト音です。Fish 音声ではありません。",
            ],
            314,
            511,
            750,
        );
        self.label(
            213,
            [
                "02 配音 — 選擇模式，逐句生成",
                "02 Speech — choose mode, generate in order",
                "02 音声 — モードを選び、順番に生成",
            ],
            16,
            562,
            520,
        );
        self.combo(MODE, &MODE_LABELS.map(|t| t[self.lang]), 16, 589, 300);
        self.label(
            214,
            ["Fish 金鑰", "Fish API key", "Fish API キー"],
            330,
            592,
            110,
        );
        self.edit(KEY, "", 444, 588, 620, 26, false, true);
        self.label(
            215,
            ["輸出資料夾", "Output folder", "出力フォルダー"],
            16,
            626,
            140,
        );
        self.edit(OUTPUT, "output", 164, 622, 532, 26, false, false);
        self.label(
            216,
            [
                "付費估價 USD / 百萬 bytes",
                "Paid USD / million bytes",
                "有料 USD / 100万 bytes",
            ],
            710,
            626,
            244,
        );
        self.edit(PRICE, "15", 962, 622, 102, 26, false, false);
        self.button(
            ONE,
            [
                "生成 / 重試此句",
                "Generate / retry line",
                "選択行を生成 / 再試行",
            ],
            16,
            659,
            195,
        );
        self.button(
            BATCH,
            ["生成全部", "Generate all", "すべて生成"],
            222,
            659,
            140,
        );
        self.button(
            CANCEL,
            ["取消佇列", "Cancel queue", "キュー取消"],
            374,
            659,
            140,
        );
        self.label(COST, ["", "", ""], 532, 665, 530);
        self.label(
            217,
            [
                "03 選用 LLM 草稿 — 先預覽，再附加；不覆寫劇本。",
                "03 Optional LLM draft — review, then append; keeps existing lines.",
                "03 任意の LLM 下書き — 確認後に追加。既存台詞は保持。",
            ],
            16,
            709,
            1048,
        );
        self.label(
            218,
            [
                "Chat completions URL",
                "Chat completions URL",
                "Chat completions URL",
            ],
            16,
            744,
            178,
        );
        self.edit(URL, "", 202, 739, 494, 26, false, false);
        self.label(219, ["模型", "Model", "モデル"], 710, 744, 65);
        self.edit(MODEL, "", 782, 739, 282, 26, false, false);
        self.label(
            220,
            ["LLM 金鑰", "LLM API key", "LLM API キー"],
            16,
            779,
            170,
        );
        self.edit(LLM_KEY, "", 202, 775, 494, 26, false, true);
        self.label(
            221,
            [
                "金鑰僅記憶體 / ENV，不儲存。",
                "Keys: memory / ENV only; never saved.",
                "キーはメモリのみ（保存しません）",
            ],
            710,
            779,
            354,
        );
        self.edit(PROMPT, "", 16, 815, 450, 90, true, false);
        self.button(
            DRAFT,
            ["產生草稿", "Generate draft", "下書き生成"],
            16,
            915,
            160,
        );
        self.edit(PREVIEW, "", 484, 815, 580, 90, true, false);
        SendMessageW(self.controls[&PREVIEW], EM_SETREADONLY, 1, 0);
        self.button(
            APPEND,
            [
                "附加預覽草稿",
                "Append reviewed draft",
                "確認した下書きを追加",
            ],
            484,
            915,
            250,
        );
        self.control(STATUS, "STATIC", "", 0, 16, 959, 1048, 44);
        if let Ok(key) = std::env::var("FISH_API_KEY") {
            let key = Zeroizing::new(key);
            self.set(KEY, &key);
        }
        if let Ok(key) = std::env::var("VOICESTAGE_LLM_KEY") {
            let key = Zeroizing::new(key);
            self.set(LLM_KEY, &key);
        }
        if let Ok(endpoint) = std::env::var("VOICESTAGE_LLM_URL") {
            self.set(URL, &endpoint);
        }
        if let Ok(model) = std::env::var("VOICESTAGE_LLM_MODEL") {
            self.set(MODEL, &model);
        }
        self.localize_choices();
        self.update_draft_state();
        self.refresh();
        self.status(self.tr([
            "就緒。先編輯劇本，再選 Mock 或 API。",
            "Ready. Edit a script, then choose Mock or API.",
            "準備完了。台詞を編集して Mock または API を選択。",
        ]));
    }
    unsafe fn refresh(&mut self) {
        self.loading = true;
        self.set(TITLE, &self.project.title);
        let speaker_items: Vec<_> = self
            .project
            .characters
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        self.combo_items(SPEAKER, &speaker_items);
        self.refresh_list();
        self.load_line();
        self.loading = false;
    }
    unsafe fn refresh_list(&self) {
        let h = self.controls[&LIST];
        SendMessageW(h, LB_RESETCONTENT, 0, 0);
        for (i, s) in self.project.segments.iter().enumerate() {
            let text = format!(
                "{:03} · {} · {}",
                i + 1,
                self.project.character(s).name,
                s.text
                    .replace(['\r', '\n'], " ")
                    .chars()
                    .take(40)
                    .collect::<String>()
            );
            SendMessageW(h, LB_ADDSTRING, 0, w(&text).as_ptr() as isize);
        }
        SendMessageW(h, LB_SETCURSEL, self.selected, 0);
        self.update_cost();
    }
    unsafe fn load_role(&self) {
        let i = self.combo_index(SPEAKER);
        if let Some(c) = self.project.characters.get(i) {
            self.set(NAME, &c.name);
            self.set(VOICE, &c.voice_id);
            self.combo_set(
                SPOKEN_LANG,
                ["zh-TW", "en", "ja", "auto"]
                    .iter()
                    .position(|x| *x == c.language)
                    .unwrap_or(0),
            );
            self.combo_set(
                EMOTION,
                EMOTIONS.iter().position(|x| *x == c.emotion).unwrap_or(0),
            );
            self.set(SPEED, &c.speed.to_string());
        }
    }
    unsafe fn load_line(&self) {
        if let Some(s) = self.project.segments.get(self.selected) {
            self.set(TEXT, &s.text);
            self.combo_set(
                SPEAKER,
                self.project
                    .characters
                    .iter()
                    .position(|c| c.id == s.character_id)
                    .unwrap_or(0),
            );
        } else {
            self.set(TEXT, "");
        }
        self.load_role();
    }
    unsafe fn apply_line(&mut self) -> Result<(), String> {
        let mut p = self.project.clone();
        p.title = self.get(TITLE);
        if let Some(s) = p.segments.get_mut(self.selected) {
            s.text = self.get(TEXT);
            s.character_id = p.characters[self.combo_index(SPEAKER)].id.clone();
        }
        p.validate()?;
        self.project = p;
        self.dirty = true;
        self.refresh_list();
        Ok(())
    }
    unsafe fn apply_role(&mut self) -> Result<(), String> {
        let mut p = self.project.clone();
        let i = self.combo_index(SPEAKER);
        let c = &mut p.characters[i];
        c.name = self.get(NAME);
        c.voice_id = self.get(VOICE);
        c.language = ["zh-TW", "en", "ja", "auto"][self.combo_index(SPOKEN_LANG)].into();
        c.emotion = EMOTIONS[self.combo_index(EMOTION)].into();
        c.speed = self.get(SPEED).parse().map_err(|_| "E_SPEED: 0.5..2.0")?;
        p.validate()?;
        self.project = p;
        self.dirty = true;
        let items: Vec<_> = self
            .project
            .characters
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        self.combo_items(SPEAKER, &items);
        self.combo_set(SPEAKER, i);
        self.refresh_list();
        Ok(())
    }
    unsafe fn update_cost(&self) {
        let mode = self.combo_index(MODE);
        EnableWindow(self.controls[&PRICE], (mode == 1) as i32);
        if mode != 1 {
            self.set(
                COST,
                if mode == 0 {
                    self.tr([
                        "本機測試音 · 不呼叫 Fish API",
                        "Local test tone · no Fish API request",
                        "ローカルテスト音 · Fish API 通信なし",
                    ])
                } else {
                    self.tr([
                        "免費模式 · 請確認供應商政策；不估算費用",
                        "Free mode · check provider policy; no estimate",
                        "無料枠 · 提供元の方針を確認。料金試算なし",
                    ])
                },
            );
            return;
        }
        let bytes = self.project.utf8_bytes();
        let price = self
            .get(PRICE)
            .parse::<f64>()
            .ok()
            .filter(|p| p.is_finite() && *p >= 0.0);
        let estimate = price.map(|p| bytes as f64 * p / 1_000_000.0);
        let Some(total) = estimate.filter(|v| v.is_finite()) else {
            self.set(
                COST,
                self.tr([
                    "估價無效 · 請輸入有限的非負費率",
                    "Invalid estimate · enter a finite, non-negative rate",
                    "試算不可 · 有限の 0 以上の単価を入力",
                ]),
            );
            return;
        };
        self.set(
            COST,
            &format!(
                "{} · {} UTF-8 bytes · ≈ US${:.6}",
                self.tr([
                    "已套用全文估價",
                    "All applied lines estimate",
                    "適用済み全行の試算",
                ]),
                bytes,
                total,
            ),
        );
    }
    unsafe fn set_busy(&self, busy: bool) {
        for id in [ONE, BATCH, DRAFT, NEW, OPEN] {
            EnableWindow(self.controls[&id], (!busy) as i32);
        }
        EnableWindow(self.controls[&CANCEL], busy as i32);
        self.update_draft_state();
    }
    unsafe fn generate(&mut self, all: bool) -> Result<(), String> {
        if self.rx.is_some() {
            return Ok(());
        }
        self.apply_role()?;
        self.apply_line()?;
        let mode = self.combo_index(MODE);
        let mock = mode == 0;
        if !mock && !self.confirm(self.tr(["即將傳送台詞及聲音 ID 到 Fish Audio。可能收費；取消不保證退款。免費模式有獨立資料政策，絕不自動切換付費。逾時或重試可能重複扣款。確定送出？","Send script and voice IDs to Fish Audio? Requests may be billed; cancellation cannot guarantee a refund. Free tier has separate data policies and never falls back to paid. Retrying unknown results may charge again. Continue?","台詞と音声 ID を Fish Audio に送信します。料金が発生する可能性があり、取消で返金は保証されません。無料枠は独自のデータ方針があり、有料への自動切替はありません。不明な結果の再試行は二重課金の可能性があります。続行？"])){return Ok(());}
        let mut p = self.project.clone();
        if !all {
            p.segments = p.segments.get(self.selected).cloned().into_iter().collect();
        }
        let config = SpeechConfig {
            model: if mode == 2 {
                "s2.1-pro-free"
            } else {
                "s2.1-pro"
            }
            .into(),
            key: Zeroizing::new(self.get(KEY)),
            mock,
        };
        let root = PathBuf::from(self.get(OUTPUT));
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.set_busy(true);
        std::thread::spawn(move || service::run_batch(p, config, root, cancel, tx));
        Ok(())
    }
    unsafe fn start_draft(&mut self) -> Result<(), String> {
        if self.rx.is_some() {
            return Ok(());
        }
        self.apply_role()?;
        self.apply_line()?;
        let c = LlmConfig {
            endpoint: self.get(URL),
            model: self.get(MODEL),
            key: Zeroizing::new(self.get(LLM_KEY)),
            prompt: self.get(PROMPT),
        };
        service::validate_endpoint(&c.endpoint)?;
        if !self.confirm(self.tr(["傳送提示與角色名稱/語言到設定的 LLM URL？可能收費。現有台詞不會被覆寫。","Send prompt and character names/language to the configured LLM URL? Charges may apply. Existing dialogue will remain intact.","プロンプトと人物名・言語を指定 LLM URL に送信しますか？料金が発生する場合があります。既存台詞は保持します。"])){return Ok(());}
        let p = self.project.clone();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.set_busy(true);
        std::thread::spawn(move || {
            let result = service::draft(&p, &c);
            if cancel.load(Ordering::Relaxed) {
                let _ = tx.send(Event::Failed(
                    "Cancelled locally; LLM billing may still apply".into(),
                ));
            } else {
                let _ = tx.send(match result {
                    Ok(d) => Event::Draft(d),
                    Err(e) => Event::Failed(e),
                });
            }
        });
        Ok(())
    }
    unsafe fn action(&mut self, id: i32) -> Result<(), String> {
        match id {
            NEW => {
                if !self.dirty
                    || self.confirm(self.tr([
                        "捨棄未儲存修改並開新範本？",
                        "Discard unsaved changes and open a new template?",
                        "未保存の変更を破棄して新規テンプレートを開きますか？",
                    ]))
                {
                    self.project = Project::template();
                    self.selected = 0;
                    self.dirty = false;
                    self.audio.clear();
                    self.candidate = None;
                    self.update_draft_state();
                    self.refresh();
                }
            }
            OPEN => {
                if !self.dirty
                    || self.confirm(self.tr([
                        "捨棄未儲存修改並開啟檔案？",
                        "Discard unsaved changes and open file?",
                        "未保存の変更を破棄してファイルを開きますか？",
                    ]))
                {
                    let p = Project::load(&PathBuf::from(self.get(PROJECT_PATH)))?;
                    self.project = p;
                    self.selected = 0;
                    self.dirty = false;
                    self.audio.clear();
                    self.candidate = None;
                    self.update_draft_state();
                    self.refresh();
                }
            }
            SAVE => {
                self.apply_role()?;
                self.apply_line()?;
                self.project.save(&PathBuf::from(self.get(PROJECT_PATH)))?;
                self.dirty = false;
                self.status(self.tr(["已儲存 JSON。", "JSON saved.", "JSON を保存しました。"]));
            }
            IMPORT => {
                self.apply_line()?;
                let path = PathBuf::from(self.get(PROJECT_PATH)).with_extension("txt");
                let text = std::fs::read_to_string(path).map_err(|_| "E_READ_TXT")?;
                if text.len() > 5_000_000 {
                    return Err("E_SIZE".into());
                }
                self.project.append_text(&text)?;
                self.dirty = true;
                self.refresh();
            }
            EXPORT => {
                self.apply_line()?;
                atomic_write(
                    &PathBuf::from(self.get(PROJECT_PATH)).with_extension("txt"),
                    self.project.export_text().as_bytes(),
                )?;
                self.status("TXT ✓");
            }
            ADD => {
                self.apply_line()?;
                let s = Segment {
                    id: new_id(),
                    character_id: self.project.characters[self.combo_index(SPEAKER)]
                        .id
                        .clone(),
                    text: self.tr(["新的台詞", "New line", "新しい台詞"]).into(),
                };
                self.selected = (self.selected + 1).min(self.project.segments.len());
                self.project.segments.insert(self.selected, s);
                self.dirty = true;
                self.refresh();
            }
            DELETE => {
                if !self.project.segments.is_empty()
                    && self.confirm(self.tr([
                        "刪除選取台詞？",
                        "Delete selected line?",
                        "選択した台詞を削除しますか？",
                    ]))
                {
                    self.project.segments.remove(self.selected);
                    self.selected = self
                        .selected
                        .min(self.project.segments.len().saturating_sub(1));
                    self.dirty = true;
                    self.refresh();
                }
            }
            UP | DOWN => {
                self.apply_line()?;
                let n = self.project.segments.len();
                if n > 0 {
                    let j = if id == UP {
                        self.selected.saturating_sub(1)
                    } else {
                        (self.selected + 1).min(n - 1)
                    };
                    self.project.segments.swap(self.selected, j);
                    self.selected = j;
                    self.refresh();
                }
            }
            ROLE_ADD => {
                self.apply_role()?;
                self.project.characters.push(Character {
                    id: new_id(),
                    name: self.tr(["新角色", "New character", "新しい人物"]).into(),
                    voice_id: String::new(),
                    language: "auto".into(),
                    emotion: "neutral".into(),
                    speed: 1.0,
                });
                self.dirty = true;
                self.refresh();
                self.combo_set(SPEAKER, self.project.characters.len() - 1);
                self.load_role();
            }
            ROLE_APPLY => self.apply_role()?,
            APPLY => self.apply_line()?,
            ONE => self.generate(false)?,
            BATCH => self.generate(true)?,
            CANCEL => {
                self.cancel.store(true, Ordering::Relaxed);
                self.status(self.tr([
                    "取消已排程；目前請求可能仍完成或收費。",
                    "Queue cancelled; an in-flight request may still finish or be billed.",
                    "キューを取消。送信済みリクエストは完了・課金される場合があります。",
                ]));
            }
            PLAY => {
                if let Some(s) = self.project.segments.get(self.selected) {
                    if let Some(path) = self.audio.get(&s.id) {
                        if PlaySoundW(
                            w(&path.to_string_lossy()).as_ptr(),
                            null_mut(),
                            SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
                        ) == 0
                        {
                            return Err("E_PLAYBACK".into());
                        }
                    } else {
                        return Err("E_NO_AUDIO: generate selected line first".into());
                    }
                }
            }
            STOP => {
                PlaySoundW(null(), null_mut(), 0);
            }
            DRAFT => self.start_draft()?,
            APPEND => {
                self.apply_line()?;
                if let Some(d) = &self.candidate {
                    d.append(&mut self.project)?;
                    self.candidate = None;
                    self.dirty = true;
                    self.update_draft_state();
                    self.refresh();
                }
            }
            _ => {}
        }
        Ok(())
    }
    unsafe fn poll(&mut self) {
        let now = Instant::now();
        self.test_max_gap = self
            .test_max_gap
            .max(now.duration_since(self.last_tick).as_millis());
        self.last_tick = now;
        self.test_ticks += 1;
        let events: Vec<_> = self
            .rx
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Status(s) => self.status(&s),
                Event::Audio(id, p) => {
                    self.audio.insert(id, p);
                }
                Event::Done(p) => {
                    self.last_run = Some(p.clone());
                    self.rx = None;
                    self.set_busy(false);
                    let summary = std::fs::read(p.join("manifest.json"))
                        .ok()
                        .and_then(|x| serde_json::from_slice::<service::Manifest>(&x).ok())
                        .map(|m| {
                            format!(
                                "{}/{} • {}",
                                m.segments.iter().filter(|s| s.status == "done").count(),
                                m.segments.len(),
                                m.segments
                                    .iter()
                                    .find(|s| s.status != "done")
                                    .map(|s| s.status.as_str())
                                    .unwrap_or("OK")
                            )
                        })
                        .unwrap_or_default();
                    self.status(&format!(
                        "{} {summary} • {}",
                        self.tr(["佇列結束", "Queue finished", "キュー完了"]),
                        p.display()
                    ));
                }
                Event::Draft(d) => {
                    self.set(
                        PREVIEW,
                        &serde_json::to_string_pretty(&d).unwrap_or_default(),
                    );
                    self.candidate = Some(d);
                    self.rx = None;
                    self.set_busy(false);
                    self.status(self.tr([
                        "草稿已備妥。檢查後按附加。",
                        "Draft ready. Review, then append.",
                        "下書きができました。確認後に追加してください。",
                    ]));
                }
                Event::Failed(e) => {
                    self.rx = None;
                    self.set_busy(false);
                    self.error(&e);
                }
            }
        }
        if self.test_dir.is_some()
            && let Err(e) = self.self_test()
        {
            let dir = self.test_dir.as_ref().unwrap();
            let _ = std::fs::write(dir.join("gui-test-failed.txt"), e);
            DestroyWindow(self.hwnd);
        }
    }
    unsafe fn scroll(&mut self, dx: i32, dy: i32) {
        let scale = GetDpiForWindow(self.hwnd) as f64 / 96.0;
        let mut rect = RECT::default();
        GetClientRect(self.hwnd, &mut rect);
        self.scroll_x =
            (self.scroll_x + dx).clamp(0, ((1080.0 * scale) as i32 - rect.right).max(0));
        self.scroll_y =
            (self.scroll_y + dy).clamp(0, ((1010.0 * scale) as i32 - rect.bottom).max(0));
        for &(h, x, y, width, height) in &self.layout {
            MoveWindow(
                h,
                (x as f64 * scale) as i32 - self.scroll_x,
                (y as f64 * scale) as i32 - self.scroll_y,
                (width as f64 * scale) as i32,
                (height as f64 * scale) as i32,
                1,
            );
        }
        for (bar, max, page, pos) in [
            (SB_HORZ, (1080.0 * scale) as i32, rect.right, self.scroll_x),
            (SB_VERT, (1010.0 * scale) as i32, rect.bottom, self.scroll_y),
        ] {
            let info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: max - 1,
                nPage: page as u32,
                nPos: pos,
                nTrackPos: 0,
            };
            SetScrollInfo(self.hwnd, bar, &info, 1);
        }
    }
    unsafe fn reveal_focus(&mut self, id: i32) {
        let Some(&target) = self.controls.get(&id) else {
            return;
        };
        let Some(&(_, x, y, width, height)) =
            self.layout.iter().find(|(h, _, _, _, _)| *h == target)
        else {
            return;
        };
        let height = if [LANG, SPEAKER, SPOKEN_LANG, EMOTION, MODE].contains(&id) {
            28
        } else {
            height
        };
        let scale = GetDpiForWindow(self.hwnd) as f64 / 96.0;
        let mut r = RECT::default();
        GetClientRect(self.hwnd, &mut r);
        let left = (x as f64 * scale) as i32 - self.scroll_x;
        let top = (y as f64 * scale) as i32 - self.scroll_y;
        let right = left + (width as f64 * scale) as i32;
        let bottom = top + (height as f64 * scale) as i32;
        let dx = if left < 0 {
            left - 8
        } else if right > r.right {
            right - r.right + 8
        } else {
            0
        };
        let dy = if top < 0 {
            top - 8
        } else if bottom > r.bottom {
            bottom - r.bottom + 8
        } else {
            0
        };
        if dx != 0 || dy != 0 {
            self.scroll(dx, dy);
        }
    }
    unsafe fn self_test(&mut self) -> Result<(), String> {
        if self.test_started.elapsed().as_secs() > 40 {
            return Err("GUI test timeout".into());
        }
        let dir = self.test_dir.clone().unwrap();
        match self.test_step {
            0 => {
                if self.controls.values().any(|h| h.is_null()) {
                    return Err("native control creation failed".into());
                }
                if self.labels.iter().any(|(_, t)| {
                    !t.iter().all(|s| !s.is_empty()) && t.iter().any(|s| !s.is_empty())
                }) {
                    return Err("translation missing".into());
                }
                self.reveal_focus(APPEND);
                self.scroll(-100000, -100000);
                std::fs::create_dir_all(&dir).map_err(|_| "test directory")?;
                self.set(PROJECT_PATH, &dir.join("project.json").to_string_lossy());
                self.set(OUTPUT, &dir.to_string_lossy());
                self.set(TEXT, "中文測試 English 日本語テスト");
                self.action(APPLY)?;
                self.action(SAVE)?;
                self.action(OPEN)?;
                if self.get(TEXT) != "中文測試 English 日本語テスト" {
                    return Err("CJK edit/save/load mismatch".into());
                }
                self.action(EXPORT)?;
                self.action(IMPORT)?;
                self.action(ADD)?;
                self.action(UP)?;
                self.action(DOWN)?;
                self.action(DELETE)?;
                self.action(BATCH)?;
                self.test_step = 1;
            }
            1 if self.rx.is_none() => {
                let path = self.last_run.as_ref().ok_or("no output")?;
                if !path.join("dialogue.wav").exists() || !path.join("dialogue.srt").exists() {
                    return Err("mix/subtitle missing".into());
                }
                self.action(PLAY)?;
                self.action(STOP)?;
                self.set(TEXT, "[MOCK_ERROR]");
                self.action(APPLY)?;
                self.action(ONE)?;
                self.test_step = 2;
            }
            2 if self.rx.is_none() => {
                let m: service::Manifest = serde_json::from_slice(
                    &std::fs::read(self.last_run.as_ref().unwrap().join("manifest.json"))
                        .map_err(|_| "manifest")?,
                )
                .map_err(|_| "manifest schema")?;
                if m.complete || !m.segments[0].status.contains("429") {
                    return Err("mock error missing".into());
                }
                self.set(TEXT, "已修正 Fixed 修正済み");
                self.action(APPLY)?;
                self.action(ONE)?;
                self.test_step = 3;
            }
            3 if self.rx.is_none() => {
                self.action(BATCH)?;
                self.action(CANCEL)?;
                self.test_step = 4;
            }
            4 if self.rx.is_none() => {
                let m: service::Manifest = serde_json::from_slice(
                    &std::fs::read(self.last_run.as_ref().unwrap().join("manifest.json"))
                        .map_err(|_| "manifest")?,
                )
                .map_err(|_| "manifest")?;
                if m.complete {
                    return Err("cancel did not stop queue".into());
                }
                self.candidate = Some(Draft {
                    segments: vec![voicestage::model::DraftLine {
                        character_id: "host".into(),
                        text: "Draft appended safely".into(),
                    }],
                });
                let n = self.project.segments.len();
                self.action(APPEND)?;
                if self.project.segments.len() != n + 1 {
                    return Err("draft append".into());
                }
                let report = serde_json::json!({"passed":true,"language":(["zh-TW","en","ja"][self.lang]),"elapsed_ms":self.test_started.elapsed().as_millis(),"ui_timer_ticks":self.test_ticks,"max_timer_gap_ms":self.test_max_gap,"checks":["CJK native edit/save/open","TXT export/import","add/delete/reorder","mock batch","WAV mix/SRT","PlaySound preview","429 failure","manual retry","cancel queue","append draft without overwrite"]});
                atomic_write(
                    &dir.join("gui-test.json"),
                    &serde_json::to_vec_pretty(&report).unwrap(),
                )?;
                self.test_step = 5;
                DestroyWindow(self.hwnd);
            }
            _ => {}
        }
        Ok(())
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        PlaySoundW(null(), null_mut(), 0);
        PostQuitMessage(0);
        return 0;
    }
    thread_local! {static ENTERED:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
    if ENTERED.with(|c| c.get()) {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ENTERED.with(|c| c.set(false));
        }
    }
    ENTERED.with(|c| c.set(true));
    let _guard = Guard;
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if msg == WM_NCCREATE {
        let cs = &*(lp as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return 1;
    }
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let app = &mut *ptr;
    match msg {
        WM_SIZE => {
            app.scroll(0, 0);
            0
        }
        WM_MOUSEWHEEL => {
            app.scroll(0, -((wp >> 16) as i16 as i32));
            0
        }
        WM_VSCROLL | WM_HSCROLL => {
            let bar = if msg == WM_VSCROLL { SB_VERT } else { SB_HORZ };
            let mut info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_ALL,
                ..Default::default()
            };
            GetScrollInfo(hwnd, bar, &mut info);
            let delta = match (wp & 0xffff) as i32 {
                SB_LINEUP => -35,
                SB_LINEDOWN => 35,
                SB_PAGEUP => -(info.nPage as i32),
                SB_PAGEDOWN => info.nPage as i32,
                SB_THUMBTRACK => info.nTrackPos - info.nPos,
                _ => 0,
            };
            if bar == SB_VERT {
                app.scroll(0, delta)
            } else {
                app.scroll(delta, 0)
            }
            0
        }
        WM_DPICHANGED => {
            let rect = &*(lp as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let scale = GetDpiForWindow(hwnd) as f64 / 96.0;
            let old = app.font;
            app.font = CreateFontW(
                -(16.0 * scale) as i32,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                w("Segoe UI").as_ptr(),
            );
            for h in app.controls.values() {
                SendMessageW(*h, WM_SETFONT, app.font as usize, 1);
            }
            DeleteObject(old);
            app.scroll(0, 0);
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xffff) as i32;
            let notification = ((wp >> 16) & 0xffff) as u32;
            if app.loading {
                return 0;
            }
            if notification == EN_SETFOCUS
                || ([LANG, SPEAKER, SPOKEN_LANG, EMOTION, MODE].contains(&id)
                    && notification == CBN_SETFOCUS)
                || (id == LIST && notification == LBN_SETFOCUS)
                || (notification == BN_SETFOCUS
                    && [
                        NEW, OPEN, SAVE, IMPORT, EXPORT, ADD, DELETE, UP, DOWN, ROLE_APPLY,
                        ROLE_ADD, APPLY, ONE, BATCH, CANCEL, PLAY, STOP, DRAFT, APPEND,
                    ]
                    .contains(&id))
            {
                app.reveal_focus(id);
                return 0;
            }
            if id == LANG && notification == CBN_SELCHANGE {
                app.lang = app.combo_index(LANG);
                for (h, t) in &app.labels {
                    SetWindowTextW(*h, w(t[app.lang]).as_ptr());
                }
                app.localize_choices();
                app.update_draft_state();
                app.update_cost();
            } else if id == MODE && notification == CBN_SELCHANGE {
                app.update_cost();
            } else if id == SPEAKER && notification == CBN_SELCHANGE {
                app.load_role();
                app.dirty = true;
            } else if id == LIST && notification == LBN_SELCHANGE {
                let next = SendMessageW(app.controls[&LIST], LB_GETCURSEL, 0, 0).max(0) as usize;
                if let Err(e) = app.apply_line() {
                    app.error(&e);
                    SendMessageW(app.controls[&LIST], LB_SETCURSEL, app.selected, 0);
                } else {
                    app.selected = next;
                    app.load_line();
                }
            } else if notification == EN_CHANGE {
                if [TITLE, NAME, VOICE, SPEED, TEXT].contains(&id) {
                    app.dirty = true;
                }
                if id == PRICE {
                    app.update_cost();
                }
            } else if notification == BN_CLICKED
                && let Err(e) = app.action(id)
            {
                app.error(&e);
            }
            0
        }
        WM_TIMER => {
            app.poll();
            0
        }
        WM_CLOSE => {
            if (app.rx.is_none()||app.confirm(app.tr(["仍有請求進行中。關閉只停止本機工作，可能仍收費。關閉？","A request is running. Closing only stops local work; charges may still apply. Close?","処理中です。終了しても送信済みリクエストは課金される場合があります。終了？"])))&&(!app.dirty||app.confirm(app.tr(["有未儲存的修改。確定關閉？","Unsaved changes. Close anyway?","未保存の変更があります。終了しますか？"]))){app.cancel.store(true,Ordering::Relaxed);DestroyWindow(hwnd);}
            0
        }
        WM_DESTROY => {
            PlaySoundW(null(), null_mut(), 0);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
pub fn run() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let args: Vec<String> = std::env::args().collect();
        let lang = args
            .iter()
            .position(|s| s == "--lang")
            .and_then(|i| args.get(i + 1))
            .map(|x| match x.as_str() {
                "en" => 1,
                "ja" => 2,
                _ => 0,
            })
            .unwrap_or(0);
        let test_dir = args
            .iter()
            .position(|s| s == "--ui-test")
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from);
        let instance = GetModuleHandleW(null());
        let cls = w("VoiceStageWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            lpszClassName: cls.as_ptr(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let now = Instant::now();
        let mut app = Box::new(App {
            hwnd: null_mut(),
            font: null_mut(),
            controls: HashMap::new(),
            labels: Vec::new(),
            lang,
            project: Project::template(),
            selected: 0,
            dirty: false,
            loading: false,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            audio: HashMap::new(),
            last_run: None,
            candidate: None,
            test_dir,
            test_step: 0,
            test_started: now,
            test_ticks: 0,
            test_max_gap: 0,
            last_tick: now,
            layout: Vec::new(),
            scroll_x: 0,
            scroll_y: 0,
        });
        let scale = GetDpiForSystem() as f64 / 96.0;
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let height = (1044.0 * scale) as i32;
        // Scrollable fixed form: vertically scroll the complete client on short displays.
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            cls.as_ptr(),
            w("VoiceStage 0.1.0").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VSCROLL | WS_HSCROLL,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            ((1100.0 * scale) as i32).min(GetSystemMetrics(SM_CXSCREEN) - 40),
            height.min(screen_h - 40),
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        app.hwnd = hwnd;
        app.font = CreateFontW(
            -(16.0 * scale) as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            w("Segoe UI").as_ptr(),
        );
        app.build();
        app.set_busy(false);
        app.dirty = false;
        app.scroll(0, 0);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *app as *mut App as isize);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        UpdateWindow(hwnd);
        SetTimer(hwnd, 1, 50, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        DeleteObject(app.font);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }
}
