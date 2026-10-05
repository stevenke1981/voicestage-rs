use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool, mpsc},
    time::Duration,
};
use voicestage::{
    audio,
    model::{Draft, Project, new_id},
    service::{self, Event, LlmConfig, Manifest, SpeechConfig},
};
use zeroize::Zeroizing;
fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!("voicestage-test-{}", new_id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn server(status: &str, body: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/tts", listener.local_addr().unwrap());
    let status = status.to_owned();
    let handle = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buf = [0; 4096];
        loop {
            let n = socket.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
            if let Some(i) = request.windows(4).position(|x| x == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..i]);
                let len = headers
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|s| s.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= i + 4 + len {
                    break;
                }
            }
        }
        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(header.as_bytes()).unwrap();
        socket.write_all(&body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, handle)
}
fn config() -> SpeechConfig {
    SpeechConfig {
        model: "s2.1-pro-free".into(),
        key: Zeroizing::new("test-only-not-a-real-key".into()),
        mock: false,
    }
}
#[test]
fn roundtrip_cjk_ids_and_atomic_replace() {
    let p = Project::template();
    let dir = temp();
    let path = dir.join("project.json");
    p.save(&path).unwrap();
    p.save(&path).unwrap();
    let loaded = Project::load(&path).unwrap();
    assert_eq!(p.segments[0].id, loaded.segments[0].id);
    assert_eq!(p.segments[1].text, loaded.segments[1].text);
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1);
}
#[test]
fn schema_rejects_unknown_duplicate_traversal() {
    let p = Project::template();
    let mut bad = p.clone();
    bad.segments[0].id = "../escape".into();
    assert!(bad.validate().is_err());
    let mut bad = p.clone();
    bad.segments[1].id = bad.segments[0].id.clone();
    assert!(bad.validate().is_err());
    let mut val = serde_json::to_value(p).unwrap();
    val["api_key"] = "private".into();
    assert!(serde_json::from_value::<Project>(val).is_err());
}
#[test]
fn language_is_metadata_emotion_speed_are_real_fields() {
    let mut p = Project::template();
    p.characters[0].emotion = "happy".into();
    p.characters[0].speed = 1.2;
    let b = service::request_body(&p, &p.segments[0]);
    assert!(b.get("language").is_none());
    assert!(b["text"].as_str().unwrap().starts_with("[happy] "));
    assert!((b["prosody"]["speed"].as_f64().unwrap() - 1.2).abs() < 0.0001);
    assert!(p.utf8_bytes() > p.segments.iter().map(|s| s.text.chars().count()).sum());
}
#[test]
fn text_import_appends_and_preserves_ids() {
    let mut p = Project::template();
    let id = p.segments[0].id.clone();
    p.append_text("guest: 日本語\nEnglish\n\n").unwrap();
    assert_eq!(p.segments.len(), 4);
    assert_eq!(p.segments[0].id, id);
    assert_eq!(p.segments[2].character_id, "guest");
}
#[test]
fn draft_is_strict_and_transactional() {
    let mut p = Project::template();
    let before = serde_json::to_vec(&p).unwrap();
    let d: Draft =
        serde_json::from_str(r#"{"segments":[{"character_id":"missing","text":"hi"}]}"#).unwrap();
    assert!(d.append(&mut p).is_err());
    assert_eq!(before, serde_json::to_vec(&p).unwrap());
    assert!(serde_json::from_str::<Draft>(r#"{"segments":[],"unexpected":1}"#).is_err());
}
#[test]
fn http_contract_wav_and_exact_model() {
    let (url, handle) = server("200 OK", audio::mock_wav());
    let p = Project::template();
    let b = service::post_tts(
        &reqwest::blocking::Client::new(),
        &url,
        &config(),
        &service::request_body(&p, &p.segments[0]),
    )
    .unwrap();
    assert_eq!(audio::samples(&b).unwrap().len(), 11025);
    let request = handle.join().unwrap();
    assert!(request.contains("model: s2.1-pro-free"));
    assert!(request.contains("content-type: application/json"));
    assert!(request.contains("\"sample_rate\":44100"));
}
#[test]
fn typo_model_blocked_before_network() {
    let mut c = config();
    c.model = "s2.1-pro-fre".into();
    let result = service::post_tts(
        &reqwest::blocking::Client::new(),
        "http://127.0.0.1:1/",
        &c,
        &serde_json::json!({}),
    );
    assert!(result.unwrap_err().starts_with("E_MODEL"));
}
#[test]
fn error_redaction_and_no_blind_retry() {
    for status in [
        "401 Unauthorized",
        "402 Payment Required",
        "422 Invalid",
        "429 Too Many Requests",
        "503 Unavailable",
    ] {
        let (url, handle) = server(status, b"secret-key private-script".to_vec());
        let result = service::post_tts(
            &reqwest::blocking::Client::new(),
            &url,
            &config(),
            &serde_json::json!({}),
        )
        .unwrap_err();
        assert!(!result.contains("secret-key"));
        assert!(!result.contains("private-script"));
        handle.join().unwrap();
    }
}
#[test]
fn loopback_llm_strict_draft() {
    let response = serde_json::json!({"choices":[{"message":{"content":"{\"segments\":[{\"character_id\":\"host\",\"text\":\"Hello 日本語\"}]}"}}]});
    let (url, handle) = server("200 OK", serde_json::to_vec(&response).unwrap());
    let c = LlmConfig {
        endpoint: url,
        model: "mock-model".into(),
        key: Zeroizing::new(String::new()),
        prompt: "Create greeting".into(),
    };
    let d = service::draft(&Project::template(), &c).unwrap();
    assert_eq!(d.segments.len(), 1);
    let request = handle.join().unwrap();
    assert!(request.contains("json_schema"));
    assert!(request.contains("\"strict\":true"));
    assert!(!request.contains("voice_id"));
}
#[test]
fn llm_endpoint_security() {
    for url in [
        "http://example.com/v1",
        "https://user:pass@example.com/v1",
        "https://example.com/v1?key=secret",
    ] {
        assert!(service::validate_endpoint(url).is_err());
    }
    assert!(service::validate_endpoint("https://example.com/v1/chat/completions").is_ok());
    assert!(service::validate_endpoint("http://localhost:1234/v1/chat/completions").is_ok());
}
fn batch(p: Project, cancel: bool) -> (PathBuf, Manifest) {
    let dir = temp();
    let (tx, rx) = mpsc::channel();
    let c = SpeechConfig {
        mock: true,
        ..config()
    };
    service::run_batch(p, c, dir, Arc::new(AtomicBool::new(cancel)), tx);
    let out = rx
        .try_iter()
        .find_map(|e| {
            if let Event::Done(p) = e {
                Some(p)
            } else {
                None
            }
        })
        .unwrap();
    let manifest =
        serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
    (out, manifest)
}
#[test]
fn batch_mix_subtitles_and_manifest() {
    let (dir, m) = batch(Project::template(), false);
    assert!(m.complete);
    assert_eq!(m.source, "MOCK_SYNTHETIC_TONE");
    assert_eq!(
        audio::samples(&std::fs::read(dir.join("dialogue.wav")).unwrap())
            .unwrap()
            .len(),
        22050
    );
    let srt = std::fs::read_to_string(dir.join("dialogue.srt")).unwrap();
    assert!(srt.contains("00:00:00,250 --> 00:00:00,500"));
    assert!(srt.contains("こんにちは"));
}
#[test]
fn cancel_before_send_and_failure_preserve_manifest() {
    let (_, m) = batch(Project::template(), true);
    assert!(!m.complete);
    assert!(m.segments.iter().all(|s| s.file.is_none()));
    let mut p = Project::template();
    p.segments[0].text = "[MOCK_ERROR]".into();
    let (dir, m) = batch(p, false);
    assert!(m.segments[0].status.contains("429"));
    assert!(!dir.join("dialogue.wav").exists());
    assert_eq!(m.segments[1].status, "not sent after previous failure");
}
#[test]
fn wav_rejects_invalid_or_truncated() {
    assert!(audio::samples(b"not audio").is_err());
    let mut data = audio::mock_wav();
    data.truncate(50);
    assert!(audio::samples(&data).is_err());
}
