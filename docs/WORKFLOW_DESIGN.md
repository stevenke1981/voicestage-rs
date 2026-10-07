# VoiceStage workflow clarity — source draft, 2026-10-07

Status: implemented in source only. Not compiled, executed, or visually verified.
Baseline: `431c9ca` (v0.1.0); working tree was clean before this change.

## Brief

For writers preparing dialogue in Traditional Chinese, English, or Japanese, the
main screen should make three decisions clear: edit the script, choose a speech
mode, and optionally request an LLM draft. The user must distinguish local test
tones from speech and a candidate draft from lines already in the project.

This round changes one layer: workflow labels and state feedback. The existing
native Windows layout, font, control geometry, JSON contract and API payloads
remain the baseline. No new dependency or drawing system is introduced.

## Design axioms

1. Use numbered task headings; avoid equally prominent decorative panels.
2. Use native system colors; avoid new gradients or custom contrast assumptions.
3. Keep Segoe UI with system CJK fallback; do not bundle fonts in this round.
4. Preserve existing control bounds; do not claim a responsive redesign.
5. Localize display choices; keep stable internal API values out of translation.
6. Name Mock as a local test tone; do not present it as generated Fish speech.
7. Show a paid estimate only in paid mode; do not infer a price for free policy.
8. Reject invalid estimate rates visibly; do not silently substitute a rate.
9. Append only an available reviewed candidate; discard it when opening a project.
10. Use static state feedback; do not add motion to imply progress or completion.

## Tokens and main-screen structure

Tokens inherit the native implementation: system `COLOR_BTNFACE`, system control
text/borders/focus, Segoe UI 16 logical pixels, 16 px outer inset, 29 px buttons,
25–26 px single-line inputs, and DPI scaling from the existing code. These are
baseline values, not newly measured accessibility guarantees. System/native
controls are the only reference studied in this round; no external visual
reference was inspected.

Main screen, in its existing reading order:

```text
VoiceStage                                        [繁體中文 / English / 日本語]
New / Open / Save / TXT             Project path + title
01 Script — apply edits
[ordered lines]                     [speaker + character settings]
                                    [line editor + apply + audio preview]
02 Speech — choose mode
[local tone / paid / free policy]   [key + output + paid rate]
[selected / all / cancel]           [mode-aware estimate or explanation]
03 Optional LLM draft
[endpoint + model + memory-only key]
[prompt + request]                  [candidate / empty guidance + append]
```

No rendered screenshot exists for this revision. This diagram is a design
description, not visual QA evidence. Long translations, CJK glyphs, clipping,
keyboard order and high-DPI behavior require future native-screen inspection.

## Source changes and rationale

- Localized mode and emotion labels retain the original index-to-API mapping.
  Language switching preserves current selection and leaves edited text intact.
- Fee feedback distinguishes local Mock, free policy (no estimate), paid estimate,
  and invalid input. Paid totals explicitly describe all applied lines; editing
  text still requires Apply, and the existing paid confirmation remains in place.
- The empty LLM preview explains the request/review/append sequence. Append is
  disabled without a candidate or while a request runs. New/Open clears the
  previous candidate only after the existing discard confirmation/load succeeds.
- Three numbered headings describe the existing workflow without moving controls.

## Verification and next gate

This round permits only source inspection/editing. Review the git diff for
translation completeness, index preservation, candidate lifecycle and scope.
Source review completed: the three mode indices and seven emotion indices still
map to the existing model/API values; language changes rebuild only choice labels;
New/Open clear candidates after the existing guards/load; request completion
updates append availability. `git diff --check` reported no whitespace errors.
The implementation change is limited to `src/ui.rs` and this design note. The
source publication step also updates README verification limits and replaces the
binary-building CI with source-only checks. This is static inspection, not a test
result or proof of the UI behavior.
Compilation, clippy, tests, mock execution, application execution and visual QA
are **NOT RUN** for this revision. Source formatting is checked separately in the
source-only publication step. Previous CI results cover only the old
implementation; they do not validate these changes. The current workflow runs
only the formatter and commit whitespace checks, with no build, test executable,
artifact upload or release job.

After the security finding is resolved with evidence and execution is explicitly
authorized again: check each locale at 100/150/200% DPI; switch locale with every
mode/emotion selected; confirm Mock/free never show a paid total; test invalid,
zero and non-finite rates; inspect preview empty/ready/busy states; exercise
New/Open/cancel/failed-load with an existing draft; verify no stale append.

Known gaps left for later: fixed-form scrolling, unsaved role fields when changing
speaker, English backend errors/status events, and lack of genuine visual QA.
The optional interface/templates skill references could not be read from their
declared resource paths; only the main design-with-intent skill was available.

The prior Google security classification is unresolved. The original Drive ZIP
was permanently deleted by the parent task (reported success and 404 readback),
and its appeal was canceled without submission. Local evidence remains retained.
Source edits, a new build or a different filename do not resolve that finding.
Binary Release remains on hold. Subsequent user authorization permits a commit
and push of this source/documentation revision to the existing repository on a
work branch; it does not permit binary publication or merging into main.
