# Performance and validation — v0.1.0

Measured 2026-10-05 on AMD Ryzen 9 3900X, Windows 11 Pro 10.0.22631, Rust 1.99.0 (`b940084d7`), x86_64-pc-windows-gnu. Four build jobs; shared workstation with unrelated workloads. This is a small application benchmark, not a controlled laboratory comparison.

## Choice

The shipping profile uses **opt-level 3, fat LTO, one codegen unit, symbol stripping and panic=abort**. Default dependency features are disabled on reqwest; only blocking-worker HTTP/JSON/Schannel TLS are enabled. Native Win32 controls/system fonts avoid shipping a web engine, GPU renderer, CJK font files or model weights. No UPX, executable compression or disabled security mechanisms.

| Candidate | EXE bytes | Median core workload | Median process start |
|---|---:|---:|---:|
| Initial baseline: opt 2 / thin LTO | 1,735,680 | 80.959 ms | 161.31 ms |
| Compact: opt s / fat LTO | 1,255,936 | 99.676 ms | 158.14 ms |
| Selected: opt 3 / fat LTO | 1,587,200 | 72.017 ms | 155.32 ms |
| Final shipping binary: opt 3 / fat LTO | 1,587,200 | 81.795 ms | 154.83 ms |

Compact saves about 21% against the selected executable but takes about 38% longer on the measured core workload. The selected profile also improves on the initial baseline in size and processing time. `opt-level=z` was not pursued: the `s` result already shows a meaningful speed tradeoff and the app is under 2 MB. This comparison varies both optimization and LTO; it does not isolate the effect of either individual flag.

The final shipping binary was rebuilt after small label/draft-handling fixes and measured again: 81.795 ms core median, 154.83 ms start median, 8,417,280-byte (~8.03 MiB) working set median. This slower core result demonstrates shared-host variability; compact is about 22% slower against that final measurement. The decision favors processing speed for an additional ~331 KB over compact, not a claim of a fixed 38% speedup. The complete ZIP is approximately 0.90 MB including license texts and documentation; exact size/hash are attached to the release.

Raw candidate data lives in [evidence](evidence/). The initial baseline was exploratory and may overlap unrelated/build activity; compact and fast were repeated after this project's profile builds ended. Final release data is recorded separately in `release-final.json`. Small differences should not be interpreted as universal performance guarantees.

## Method and limits

`scripts/build.ps1` builds with compiler source-path remapping. `scripts/benchmark.ps1 -Profile <profile>` launches five new processes, measures from immediately before Start-Process to WaitForInputIdle, then reads working set and private bytes after 1.5 seconds. Each process closes normally. No foreground input is used. The **OS file cache is not flushed**, so this is process-cold startup, not disk-cold or post-reboot startup; presentation-to-photon latency is not measured. An earlier exploratory run had a ~700 ms outlier on the shared host; all raw final trials are retained rather than claiming a hard latency bound.

Core benchmark: five trials, each 200 iterations of parsing, validating, serializing and UTF-8 counting a 500-line CJK script. Each trial processes 100,000 lines. It does not estimate network or Fish inference speed. `--benchmark-core <json-path>` reproduces it without opening the GUI. The selected candidate's idle working set median was 8,392,704 bytes (~8.00 MiB), private bytes 1,683,456 (~1.61 MiB).

The GUI polls worker events every 50 ms. Three-language `--ui-test` reports include observed timer gaps across startup, native control edits, synchronous project file actions and the mock queue. These gaps are not frame times; they include test setup and disk work. Each speech response is capped at 50 MB. Batch mixing reads one segment at a time and streams the combined PCM into a new WAV; no whole-project audio accumulation. Concurrency is one.

## Validation

- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and 13 core tests pass locally.
- Loopback mock tests cover exact Fish headers/body, model typo rejection before network, CJK/UTF-8 costs, secret/body error redaction, non-retry behavior, strict LLM schema and endpoint policy, append-only drafts, stable IDs, transactional saves, WAV validation/mixing/SRT, cancellation and failure manifests.
- Native-control tests launch actual zh-TW, English and Japanese windows and exercise editing, JSON save/open, TXT import/export, add/delete/reorder, mock batch, WAV/SRT, PlaySound API preview, mock 429, manual retry, cancellation and draft append. Language/layout/control creation checks are included. Reports are in `evidence/gui-*.json`. Playback API success does not prove speaker output was heard.
- PE inspection confirms x64, DEP/NX, ASLR and high-entropy ASLR. Imports are Windows OS DLLs only. Targeted secret/private-path scan passes. Dependency notices and upstream license texts are bundled. This targeted scan is not a guarantee against every possible secret format.

**Not executed:** real Fish/LLM API calls; actual paid/free service billing behavior; independent screenshot/mouse/keyboard visual inspection; multiple physical DPI configurations; auditory quality verification. Supported desktop automation was unavailable and the supported capture service refused connection. Programmatic native-control self-tests do not prove visual layout or CJK glyph appearance. Visual acceptance remains with the user. No alternative desktop capture/control mechanism was used to bypass that limitation.

The portable ZIP includes all three quick starts, release notes, example JSON, evidence and full license notices. `SHA256SUMS.txt` identifies the exact distributed ZIP/executable; the ZIP itself contains checksums for its files. ZIP CRC and extracted executable equality are checked during packaging.
