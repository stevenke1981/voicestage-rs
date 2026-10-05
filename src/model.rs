use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub fn new_id() -> String {
    format!(
        "s{:x}-{:x}-{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub voice_id: String,
    /// Editorial language hint. Fish automatically detects speech language.
    pub language: String,
    pub emotion: String,
    pub speed: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub id: String,
    pub character_id: String,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub version: u32,
    pub title: String,
    pub characters: Vec<Character>,
    pub segments: Vec<Segment>,
}
pub const EMOTIONS: &[&str] = &[
    "neutral",
    "happy",
    "sad",
    "angry",
    "excited",
    "whispering",
    "calm",
];
fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
impl Project {
    pub fn template() -> Self {
        Self {
            version: 1,
            title: "VoiceStage".into(),
            characters: vec![
                Character {
                    id: "host".into(),
                    name: "主持人 / Host / 司会".into(),
                    voice_id: String::new(),
                    language: "zh-TW".into(),
                    emotion: "neutral".into(),
                    speed: 1.0,
                },
                Character {
                    id: "guest".into(),
                    name: "來賓 / Guest / ゲスト".into(),
                    voice_id: String::new(),
                    language: "ja".into(),
                    emotion: "happy".into(),
                    speed: 1.0,
                },
            ],
            segments: vec![
                Segment {
                    id: new_id(),
                    character_id: "host".into(),
                    text: "你好！Welcome to VoiceStage.".into(),
                },
                Segment {
                    id: new_id(),
                    character_id: "guest".into(),
                    text: "こんにちは！一起開始寫劇本吧。".into(),
                },
            ],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.title.len() > 1024
            || self.characters.is_empty()
            || self.characters.len() > 100
            || self.segments.len() > 5000
        {
            return Err("E_SCHEMA: version / project limits".into());
        }
        let mut ids = HashSet::new();
        for c in &self.characters {
            if !safe_id(&c.id)
                || !ids.insert(&c.id)
                || c.name.trim().is_empty()
                || c.name.len() > 300
                || c.voice_id.len() > 200
                || !c
                    .voice_id
                    .bytes()
                    .all(|x| x.is_ascii_alphanumeric() || x == b'-' || x == b'_')
                || !["zh-TW", "en", "ja", "auto"].contains(&c.language.as_str())
                || !EMOTIONS.contains(&c.emotion.as_str())
                || !c.speed.is_finite()
                || !(0.5..=2.0).contains(&c.speed)
            {
                return Err("E_CHARACTER: invalid or duplicate character/settings".into());
            }
        }
        let mut segments = HashSet::new();
        for s in &self.segments {
            if !safe_id(&s.id)
                || !segments.insert(&s.id)
                || !ids.contains(&s.character_id)
                || s.text.trim().is_empty()
                || s.text.len() > 10000
            {
                return Err(
                    "E_SEGMENT: invalid ID, character or text (1..10000 UTF-8 bytes)".into(),
                );
            }
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        if fs::metadata(path).map_err(|_| "E_READ")?.len() > 10_000_000 {
            return Err("E_SIZE".into());
        }
        let data = fs::read(path).map_err(|_| "E_READ".to_string())?;
        if data.len() > 10_000_000 {
            return Err("E_SIZE".into());
        }
        let p: Self = serde_json::from_slice(&data)
            .map_err(|_| "E_SCHEMA: invalid project JSON".to_string())?;
        p.validate()?;
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        atomic_write(
            path,
            &serde_json::to_vec_pretty(self).map_err(|_| "E_JSON")?,
        )
    }
    pub fn character(&self, s: &Segment) -> &Character {
        self.characters
            .iter()
            .find(|c| c.id == s.character_id)
            .expect("validated project")
    }
    pub fn spoken_text(&self, s: &Segment) -> String {
        let c = self.character(s);
        if c.emotion == "neutral" {
            s.text.clone()
        } else {
            format!("[{}] {}", c.emotion, s.text)
        }
    }
    pub fn utf8_bytes(&self) -> usize {
        self.segments
            .iter()
            .map(|s| self.spoken_text(s).len())
            .sum()
    }
    pub fn append_text(&mut self, text: &str) -> Result<usize, String> {
        let mut next = self.clone();
        let before = next.segments.len();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (id, text) = match line.split_once(':') {
                Some((id, t)) if next.characters.iter().any(|c| c.id == id.trim()) => {
                    (id.trim().to_owned(), t.trim().to_owned())
                }
                _ => (next.characters[0].id.clone(), line.trim().to_owned()),
            };
            next.segments.push(Segment {
                id: new_id(),
                character_id: id,
                text,
            });
        }
        next.validate()?;
        let n = next.segments.len() - before;
        *self = next;
        Ok(n)
    }
    pub fn export_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| format!("{}: {}", s.character_id, s.text.replace('\n', " ")))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Write to an adjacent new file, then replace atomically on Windows/Unix.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let temp = path.with_extension(format!("{}.part", new_id()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "E_WRITE")?;
        f.write_all(data)
            .and_then(|_| f.sync_all())
            .map_err(|_| "E_WRITE")?;
        drop(f);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
            }
            let a: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let b: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 0x1 | 0x8) } == 0 {
                return Err("E_RENAME");
            }
        }
        #[cfg(not(windows))]
        fs::rename(&temp, path).map_err(|_| "E_RENAME")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(str::to_owned)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub segments: Vec<DraftLine>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DraftLine {
    pub character_id: String,
    pub text: String,
}
impl Draft {
    pub fn append(&self, p: &mut Project) -> Result<(), String> {
        if self.segments.is_empty() || self.segments.len() > 200 {
            return Err("E_DRAFT: 1..200 lines required".into());
        }
        let mut next = p.clone();
        next.segments.extend(self.segments.iter().map(|s| Segment {
            id: new_id(),
            character_id: s.character_id.clone(),
            text: s.text.clone(),
        }));
        next.validate()?;
        *p = next;
        Ok(())
    }
}
