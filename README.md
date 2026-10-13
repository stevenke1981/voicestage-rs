> [!WARNING]
> **本專案已封存（唯讀），不再維護。** 後續開發請改至：**https://github.com/stevenke1981/fish-s2pro-tts**
>
> **This repository is archived (read-only) and no longer maintained.** Continued development: **https://github.com/stevenke1981/fish-s2pro-tts**

# VoiceStage

**Binary release on hold (2026-10-05):** Google Drive blocked download of the prepared portable ZIP with `cannotDownloadAbusiveFile` (malware/spam classification). No GitHub executable release attachment has been published. Local Defender found no threats, but that does not resolve Google's finding or establish a false positive. See [distribution status](docs/DISTRIBUTION_STATUS.md). Source and completed CI remain available; do not treat the prepared binary as cleared for distribution.

A lightweight Rust desktop app for character dialogue scripts and Fish Audio S2.1 Pro speech. Windows 11 x64 first. 繁體中文 / English / 日本語 switch instantly inside the window.

**v0.1.0 uses independently written client code. Real Fish/LLM requests have not been exercised for this release. Mock produces a short synthetic 440 Hz tone, never claimed to be Fish speech.** No model weights, Python, Electron, WebView or GPU runtime are bundled.

## Start

Download the portable ZIP from [Releases](https://github.com/stevenke1981/voicestage-rs/releases), extract it into a writable folder, and run `voicestage.exe`. No installer or administrator access required. The executable is unsigned. Preserve the included license notices.

1. Start with the included two-character template. Select a line on the left; edit its text and apply it.
2. Select a character, edit name, your authorized voice ID, language hint, emotion and speed (0.5–2.0), then apply the character. Language is an editorial hint; Fish detects the speech language from text. Emotion is sent as an S2 square-bracket prefix.
3. Set a project JSON path and save. Open reads that path. TXT append/export uses the same path with `.txt`; one `character_id: text` line per utterance. TXT import appends and assigns new IDs. JSON preserves IDs and rejects unknown fields, duplicates and path-like IDs. Apply a character before selecting another character.
4. Leave **Mock** selected to test offline. Set an output folder, generate one line or all, and preview the selected WAV. `[MOCK_ERROR]` in a line simulates HTTP 429. Remove it and explicitly retry that line.
5. For real speech, choose the exact paid or free model, enter your own key and authorized voice IDs, then review the send confirmation. This is an explicit external request and can cost money. Never submit third-party voices without rights.

The form scrolls vertically/horizontally on smaller screens. Native controls provide keyboard Tab navigation, selection, clipboard and IME input; Windows system fonts provide CJK fallback. The UI is per-monitor DPI aware. The app starts without stealing focus.

## Draft writing and speech are separate

Fish is a speech synthesizer, not a script-writing LLM. Manual editing/templates work without any key. Optional draft generation accepts a full OpenAI-compatible `/v1/chat/completions` URL, model, key and prompt. HTTPS is required except loopback HTTP for local providers. Providers must support `response_format: json_schema` with strict schema; unsupported providers fail explicitly. There is no silent fallback to unstructured output. A returned draft is validated, displayed separately, then **appended only after review**. Existing text is never replaced by the LLM. Requests send the prompt and character names/language; existing dialogue and voice IDs are not sent to the LLM.

## Outputs and cancellation

Each run writes a new `run-…` directory with segment WAVs and `manifest.json`. A complete batch also writes `dialogue.wav` and segment-timed `dialogue.srt`. Subtitle boundaries use actual decoded PCM sample counts, not speech recognition or word alignment. Concatenation has no inserted pauses. Incomplete batches retain successful segments and a manifest; they do not create a misleading complete mix. Mixed audio is streamed to disk, with at most one decoded segment held for mixing.

Concurrency is **1** to respect shared account quota and keep ordering deterministic. Networking runs on a background worker; the UI polls messages every 50 ms. Cancel stops dispatch of further segments. An in-flight HTTP request is allowed to finish (120 s timeout) so its returned audio is preserved; cancelling does **not** promise server cancellation or a refund. Closing during a request can leave an in-flight manifest entry; check your account before retrying. No POST, including 429, is automatically retried. HTTP 401/402/403/404/422/429 are surfaced. Transport errors, 5xx and malformed/incomplete successful audio are treated as potentially billed/UNKNOWN. Retry only the selected line after checking uncertain results; every run is a separate request.

## Keys and privacy

Keys are masked in the native edit controls and held in memory. Optional existing environment variables: `FISH_API_KEY`, `VOICESTAGE_LLM_KEY`, `VOICESTAGE_LLM_URL`, `VOICESTAGE_LLM_MODEL`. No key persistence, telemetry or app log is implemented. Project JSON contains only script data. Output manifests intentionally contain the dialogue so users can audit their own generation. Keep private projects/output out of public repositories. Secret strings copied into worker configuration are zeroized on drop; native control buffers, OS paging or a debugger are not a secure vault.

HTTP redirects and automatic retries are disabled. TLS uses Windows Schannel and the system trust store, with certificate validation enabled. Fish endpoint is fixed to `https://api.fish.audio/v1/tts`. Only `s2.1-pro` and `s2.1-pro-free` are allowed. Unknown models are blocked locally because the service otherwise falls back to paid. Free never silently upgrades to paid. No voice upload/cloning route is provided.

Paid estimate defaults to US$15 per million UTF-8 bytes and can be edited. It counts transmitted emotion-prefix bytes too; this is an estimate, not a billing guarantee. Free tier availability and data policies can change. Review the official sources before sending sensitive content:

- [Fish TTS contract](https://docs.fish.audio/api-reference/endpoint/openapi-v1/text-to-speech)
- [Pricing and rate limits](https://docs.fish.audio/developer-guide/models-pricing/pricing-and-rate-limits)
- [Free API policy](https://fish.audio/blog/s2-1-pro-free-api/)
- [Terms](https://fish.audio/terms/)

## Build and verify

Rust stable, Windows x64 toolchain (GNU or MSVC). `Cargo.lock` is committed. GNU builds need the standard Rust toolchain's linker. Limit jobs on shared workstations:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -j 4 -- -D warnings
cargo test --locked -j 4
./scripts/build.ps1
.\target\release\voicestage.exe --lang en
.\target\release\voicestage.exe --lang ja --ui-test .\test-output\ja
```

`--ui-test <directory>` executes native-control integration checks with synthetic audio only, saves a JSON report, then exits. It is a test harness using production action handlers, not a substitute for independent mouse/keyboard or screenshot QA. It may produce a brief test tone. Tests use disposable paths and loopback mock HTTP; no paid services are required.

See [performance](docs/PERFORMANCE.md), [release notes](docs/RELEASE_NOTES.md), [繁體中文](docs/QUICKSTART.zh-TW.md), [日本語](docs/QUICKSTART.ja.md), and [third-party notices](THIRD_PARTY_NOTICES.md).

## Scope

v0.1.0 supports per-line synthesis, not native one-request multi-speaker synthesis. It does not offer CSV, voice cloning, voice browsing, word-level subtitles, automatic retries, encrypted credential storage, autosave or undo history. These are not presented as completed features. Large imported files are bounded (10 MB JSON, 5 MB TXT, 5,000 lines); each speech response is limited to 50 MB and LLM response to 1 MB.
