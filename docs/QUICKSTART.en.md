# VoiceStage quick start

Extract the portable ZIP into a writable folder on Windows 11 x64 and run `voicestage.exe`. Choose Traditional Chinese, English or Japanese at the top right. Scroll the form on smaller screens; Tab moves keyboard focus.

1. Select a line on the left, edit its text, then Apply line. Add/delete/reorder lines as needed. Edit character name, authorized voice ID, language hint, emotion and speed, then Apply role before switching characters. Fish automatically detects spoken language from text.
2. Enter a project `.json` path and Save JSON. Open JSON reads that path. TXT import/export uses the same base name with `.txt`, one `character_id: text` line per utterance. TXT appends; JSON preserves segment IDs. Save often: no autosave or undo history.
3. Default Mock generates a short synthetic 440 Hz tone, **not Fish speech**. Choose an output directory, generate one or all lines, then preview the selected audio. `[MOCK_ERROR]` simulates 429; remove it and retry the selected line explicitly.
4. For actual speech, select the paid or free exact model, enter your own Fish key and authorized voice IDs, and review the send confirmation. Keys stay in memory or existing ENV (`FISH_API_KEY`); no credential saving. Free never silently falls back to paid.
5. Optional LLM writing is separate from TTS. Enter the full chat completions URL, model, key and prompt. The provider must support strict JSON schema. Review the candidate, then Append reviewed draft. Existing dialogue is never overwritten. Manual editing/templates need no key.

Each run has segment WAVs and a manifest. A complete batch additionally has a combined WAV and SRT timed from actual segment durations, not word recognition. Cancellation stops the pending queue; an in-flight request can finish or be billed (120 s timeout). UNKNOWN results may already be billed; check the account before retrying. No POST is automatically retried. The editable paid estimate is based on UTF-8 bytes including emotion tags.

This release was tested with local mock HTTP and generated tones. **Live Fish/LLM API verification and independent visual acceptance remain pending.** Review the free tier's current data policy before submitting private content. Voice upload/cloning, CSV and native one-request multi-speaker synthesis are not included. Preserve the bundled licenses.
