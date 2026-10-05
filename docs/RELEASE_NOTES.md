# VoiceStage v0.1.0

Initial independently written Windows 11 x64 portable release.

- Native Win32 Rust interface, instant Traditional Chinese / English / Japanese switching, system CJK fonts, DPI-aware scrollable form.
- Character/voice ID/language hint/emotion/speed editing; stable segment IDs; validated JSON; append-only TXT import and text export.
- Optional strict-schema OpenAI-compatible draft generation with separate preview and explicit append.
- Fixed official Fish TTS endpoint, exact paid/free allowlist, no paid fallback, masked memory-only keys.
- Background single-worker batch/single-line generation, queue cancellation, explicit selected-line retry, synthetic mock mode and failure simulation.
- WAV playback, per-segment export, manifest, PCM-safe WAV concatenation and segment-duration SRT on complete batches.
- Tests use local mock servers and generated tones. **No live Fish or LLM request was made.** API compatibility is based on the official documentation, not a paid end-to-end verification.

Limitations: Windows only; unsigned executable; per-line synthesis only; cancellation cannot cancel/refund an already submitted request; no automatic retries, autosave, undo history, voice uploads, word-level alignment or saved credentials. LLM providers must support strict JSON schema. Native UI integration test coverage and visual QA evidence are recorded in PERFORMANCE.md.

**Visual acceptance pending:** Native-control self-tests passed in all three languages. Independent screenshot, mouse/keyboard and multi-DPI visual inspection could not run because the supported Windows desktop automation entry point was unavailable and the supported capture service was not running. Programmatic control tests do not establish visual correctness or CJK glyph appearance. Please treat this first release as requiring user visual acceptance, especially with display scaling/IME. No screenshots or manual QA pass are claimed.
