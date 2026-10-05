use crate::{
    audio,
    model::{Draft, Project, Segment, atomic_write, new_id},
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    time::Duration,
};
use zeroize::Zeroizing;

pub const FISH_URL: &str = "https://api.fish.audio/v1/tts";
pub const MODELS: &[&str] = &["s2.1-pro", "s2.1-pro-free"];
pub struct SpeechConfig {
    pub model: String,
    pub key: Zeroizing<String>,
    pub mock: bool,
}
#[derive(Debug)]
pub enum Event {
    Status(String),
    Audio(String, PathBuf),
    Done(PathBuf),
    Draft(Draft),
    Failed(String),
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub version: u32,
    pub source: String,
    pub model: String,
    pub segments: Vec<Entry>,
    pub complete: bool,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
    pub id: String,
    pub character_id: String,
    pub text: String,
    pub status: String,
    pub file: Option<String>,
    pub utf8_bytes: usize,
}

pub fn request_body(p: &Project, s: &Segment) -> Value {
    let c = p.character(s);
    json!({"text":p.spoken_text(s),"reference_id":c.voice_id,"format":"wav","sample_rate":44100,"latency":"normal","prosody":{"speed":c.speed,"volume":0,"normalize_loudness":true},"normalize":true})
}
fn client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| "E_CLIENT".into())
}
/// Never retries a POST. A transport failure after send may already have been billed.
pub fn post_tts(
    client: &Client,
    url: &str,
    config: &SpeechConfig,
    body: &Value,
) -> Result<Vec<u8>, String> {
    if !MODELS.contains(&config.model.as_str()) {
        return Err("E_MODEL: blocked before request".into());
    }
    let response = client
        .post(url)
        .bearer_auth(config.key.as_str())
        .header("model", &config.model)
        .json(body)
        .send()
        .map_err(|_| {
            "UNKNOWN: transport/timeout; request may be billed; no automatic retry".to_string()
        })?;
    let code = response.status().as_u16();
    if code != 200 {
        return Err(match code {
            401 => "HTTP 401: invalid credentials",
            402 => "HTTP 402: insufficient balance",
            403 => "HTTP 403: access denied",
            404 => "HTTP 404: voice/endpoint not found",
            422 => "HTTP 422: invalid input",
            429 => "HTTP 429: rate limited; wait before manual retry",
            500..=599 => "UNKNOWN: server error; check account before manual retry",
            _ => "HTTP error: request rejected",
        }
        .into());
    }
    let mut data = Vec::new();
    response
        .take(50_000_001)
        .read_to_end(&mut data)
        .map_err(|_| "UNKNOWN: incomplete response; may be billed")?;
    if data.len() > 50_000_000 {
        return Err("UNKNOWN: audio exceeds 50 MB limit".into());
    }
    audio::samples(&data).map_err(|_| "UNKNOWN: invalid/incomplete WAV; request may be billed")?;
    Ok(data)
}
pub fn run_batch(
    p: Project,
    config: SpeechConfig,
    root: PathBuf,
    cancel: Arc<AtomicBool>,
    tx: Sender<Event>,
) {
    let result = (|| -> Result<PathBuf, String> {
        p.validate()?;
        if !config.mock
            && (!MODELS.contains(&config.model.as_str())
                || config.key.trim().is_empty()
                || p.segments
                    .iter()
                    .any(|s| p.character(s).voice_id.is_empty()))
        {
            return Err("E_CONFIG: model, key and voice IDs required".into());
        }
        if p.segments.is_empty() {
            return Err("E_EMPTY".into());
        }
        let dir = root.join(format!("run-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|_| "E_DIRECTORY")?;
        let mut manifest = Manifest {
            version: 1,
            source: if config.mock {
                "MOCK_SYNTHETIC_TONE"
            } else {
                "FISH_API_UNVERIFIED_CLIENT"
            }
            .into(),
            model: if config.mock {
                "mock".into()
            } else {
                config.model.clone()
            },
            segments: p
                .segments
                .iter()
                .map(|s| Entry {
                    id: s.id.clone(),
                    character_id: s.character_id.clone(),
                    text: s.text.clone(),
                    status: "queued".into(),
                    file: None,
                    utf8_bytes: p.spoken_text(s).len(),
                })
                .collect(),
            complete: false,
        };
        let manifest_path = dir.join("manifest.json");
        save_manifest(&manifest_path, &manifest)?;
        let client = client()?;
        let mut mixes = Vec::new();
        for (i, s) in p.segments.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            manifest.segments[i].status = "in-flight; billing unknown if interrupted".into();
            save_manifest(&manifest_path, &manifest)?;
            let _ = tx.send(Event::Status(format!(
                "{}/{} • {}",
                i + 1,
                p.segments.len(),
                s.id
            )));
            let result = if config.mock {
                for _ in 0..10 {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(30));
                }
                if s.text.contains("[MOCK_ERROR]") {
                    Err("HTTP 429: MOCK simulated failure".into())
                } else {
                    Ok(audio::mock_wav())
                }
            } else {
                post_tts(&client, FISH_URL, &config, &request_body(&p, s))
            };
            match result {
                Ok(data) => {
                    let filename = format!("{}.wav", s.id);
                    let file = dir.join(&filename);
                    atomic_write(&file, &data)?;
                    mixes.push((s.id.clone(), s.text.clone(), file.clone()));
                    manifest.segments[i].status = "done".into();
                    manifest.segments[i].file = Some(filename);
                    let _ = tx.send(Event::Audio(s.id.clone(), file));
                }
                Err(e) => {
                    manifest.segments[i].status = e.clone();
                    save_manifest(&manifest_path, &manifest)?;
                    let _ = tx.send(Event::Status(e));
                    break;
                }
            }
            save_manifest(&manifest_path, &manifest)?;
        }
        for entry in &mut manifest.segments {
            if entry.status == "queued" {
                entry.status = if cancel.load(Ordering::Relaxed) {
                    "cancelled locally"
                } else {
                    "not sent after previous failure"
                }
                .into();
            }
        }
        manifest.complete = manifest.segments.iter().all(|e| e.status == "done");
        if manifest.complete {
            audio::write_mix(&dir, &mixes)?;
        }
        save_manifest(&manifest_path, &manifest)?;
        Ok(dir)
    })();
    match result {
        Ok(p) => {
            let _ = tx.send(Event::Done(p));
        }
        Err(e) => {
            let _ = tx.send(Event::Failed(e));
        }
    }
}
fn save_manifest(path: &Path, m: &Manifest) -> Result<(), String> {
    atomic_write(path, &serde_json::to_vec_pretty(m).map_err(|_| "E_JSON")?)
}

pub struct LlmConfig {
    pub endpoint: String,
    pub model: String,
    pub key: Zeroizing<String>,
    pub prompt: String,
}
pub fn validate_endpoint(s: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(s).map_err(|_| "E_URL")?;
    if !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("E_URL: no credentials/query/fragment in endpoint".into());
    }
    if u.scheme() != "https"
        && !(u.scheme() == "http"
            && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
    {
        return Err("E_URL: HTTPS or loopback HTTP required".into());
    }
    Ok(())
}
pub fn draft(p: &Project, c: &LlmConfig) -> Result<Draft, String> {
    validate_endpoint(&c.endpoint)?;
    if c.model.trim().is_empty() || c.prompt.trim().is_empty() {
        return Err("E_LLM_CONFIG".into());
    }
    let ids: Vec<_> = p.characters.iter().map(|c| c.id.as_str()).collect();
    let schema = json!({"type":"object","additionalProperties":false,"required":["segments"],"properties":{"segments":{"type":"array","minItems":1,"maxItems":200,"items":{"type":"object","additionalProperties":false,"required":["character_id","text"],"properties":{"character_id":{"type":"string","enum":ids},"text":{"type":"string","minLength":1,"maxLength":5000}}}}}});
    let body = json!({"model":c.model,"messages":[{"role":"system","content":format!("Write a dialogue draft as strict JSON, using these character IDs and editorial language hints: {}. Return only an object with segments array, each with character_id and text. Do not include API keys or claim to synthesize audio.",serde_json::to_string(&p.characters.iter().map(|x|json!({"id":x.id,"name":x.name,"language":x.language})).collect::<Vec<_>>()).map_err(|_|"E_JSON")?)},{"role":"user","content":c.prompt}],"response_format":{"type":"json_schema","json_schema":{"name":"dialogue","strict":true,"schema":schema}}});
    let mut request = client()?.post(&c.endpoint).json(&body);
    if !c.key.is_empty() {
        request = request.bearer_auth(c.key.as_str());
    }
    let response = request
        .send()
        .map_err(|_| "UNKNOWN: LLM request may be billed; no automatic retry")?;
    if !response.status().is_success() {
        return Err(format!(
            "HTTP {}: LLM rejected request (strict JSON schema support required)",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(1_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "UNKNOWN: incomplete LLM response")?;
    if bytes.len() > 1_000_000 {
        return Err("E_LLM_SIZE".into());
    }
    let outer: Value = serde_json::from_slice(&bytes).map_err(|_| "E_LLM_JSON")?;
    let text = outer["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("E_LLM_CONTENT")?;
    let candidate: Draft = serde_json::from_str(text).map_err(|_| "E_DRAFT_SCHEMA")?;
    candidate.append(&mut p.clone())?;
    Ok(candidate)
}
