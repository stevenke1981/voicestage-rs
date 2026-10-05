# v0.1.0 distribution hold

Recorded 2026-10-05. The source repository is public. The executable Release attachment is **not published**, and the prepared portable binary is held pending investigation of the provider's security finding.

Google Drive accepted the upload into the requested private account. Metadata readback showed a single owner permission and the expected ZIP size. A subsequent authenticated download readback failed:

```text
HTTP 403
reason: cannotDownloadAbusiveFile
message: This file has been identified as malware or spam and cannot be downloaded.
```

No abuse override was used. The ZIP was not renamed, repackaged, uploaded to another host as a workaround, or executed again after the finding. The local artifact and provider response were retained. Hash/version-note text files remained readable and private. No new credential grant or sharing permission was created.

## Static and scanner evidence

- Original implementation/build source commit: `6bd57d1663abd8f035abf51cfb669444cd0bed84`.
- [CI for that exact implementation](https://github.com/stevenke1981/voicestage-rs/actions/runs/37355332243): **completed / success**. Formatting, clippy, 13 tests, release build and targeted PE/path/secret audit passed.
- ZIP: 903,919 bytes; SHA-256 `224a097c538409abae90c3729ab1cf343a96612bed310f0262707395c38046ec`.
- EXE: 1,587,200 bytes; SHA-256 `a6e43c40dba416599b6d20dc7ff86f415eed347979aae37259a9f14b67d82324`.
- Extracted EXE bytes equal the local release build; embedded BUILD_INFO identifies the implementation commit above.
- All 120 cached registry crate archives matched their Cargo.lock SHA-256 checksums. This establishes consistency with the lockfile, not an independent audit of every dependency.
- Microsoft Defender scan-only custom scans of the exact ZIP and EXE both exited 0 and reported **found no threats**. Engine `1.1.26080.3`, signature `1.459.560.0`, signature timestamp `2026-10-05T06:15:51+08:00`, antivirus enabled. Scanning did not run the application or change protection settings; remediation was disabled for these diagnostic scans, not antivirus protection.
- PE audit retained DEP/NX, ASLR and high-entropy ASLR; imports are Windows system DLLs. Targeted private-path/credential-format scans passed before upload.

These results **do not explain or overturn Google's classification**. A false positive is not established. The ZIP's download has not been verified and the binary release is incomplete. The account owner may need to use Google's official review/appeal process; publication should remain on hold until the security finding is resolved. No third-party scanning upload was performed.

Independent visual acceptance and live Fish/LLM API verification also remain pending, as documented in the release notes. Native-control/mock tests were completed **before** the provider finding; no further application execution was performed afterward.

The follow-up repository commit changes only this status documentation and README. It skips redundant CI deliberately; the implementation code and originally verified CI commit remain unchanged. The original ZIP is preserved, not rebuilt to conceal the provider finding.
