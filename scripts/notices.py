"""Generate dependency attribution from cargo metadata. Does not publish local paths."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=root))
lines = ["# Third-party notices", "", "VoiceStage code is MIT. Dependencies retain their own licenses.", "This list conservatively includes all resolved Cargo packages (including build/other-platform packages not shipped in the Windows executable).", "Dependency source is available from the versioned crates.io links below; Cargo.lock pins source/checksums. No dependency source is modified.", "", "Hound is MPL-2.0: its unmodified corresponding source is available at the versioned source link below. VoiceStage's own code is separate and MIT.", ""]
texts = ["Third-party license texts for VoiceStage\n"]
missing = []
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["name"] == "voicestage":
        continue
    name, version = package["name"], package["version"]
    license_name = package.get("license") or "see license file"
    lines.append(f"- **{name} {version}** — {license_name}; [source](https://crates.io/crates/{name}/{version})")
    directory = pathlib.Path(package["manifest_path"]).parent
    candidates = [p for p in directory.iterdir() if p.is_file() and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"))]
    for sub in (directory / "LICENSES", directory / "licenses"):
        if sub.is_dir():
            candidates.extend(p for p in sub.iterdir() if p.is_file())
    if package.get("license_file"):
        candidates.append(directory / package["license_file"])
    if name == "r-efi" and not candidates:
        # This non-Windows transitive package ships SPDX/README/AUTHORS but no
        # license text files. Elect Apache-2.0 (full text included below).
        candidates.extend([directory / "README.md", directory / "AUTHORS"])
        lines.append("  Apache-2.0 option elected; package README/AUTHORS and full Apache-2.0 text are included. Not linked into the Windows build.")
    if not candidates:
        missing.append(f"{name} {version}")
    for p in sorted(set(candidates)):
        texts.append(f"\n{'=' * 72}\n{name} {version} — {p.name}\n{'=' * 72}\n{p.read_text(encoding='utf-8', errors='replace')}")
sysroot = pathlib.Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
rust_doc = sysroot / "share/doc/rust"
lines.extend(["", "## Rust runtime and compiler support", "", "Rust standard library/compiler support carries its upstream licenses, principally MIT/Apache-2.0 and applicable exceptions. Included `RUST-COPYRIGHT-library.html` preserves the toolchain's standard-library notices; `THIRD_PARTY_LICENSES.txt` also includes its supplied license texts.", "Windows system DLLs and fonts are supplied by the user's OS, not redistributed."])
for p in sorted((rust_doc / "licenses").glob("*.txt")):
    texts.append(f"\nRust toolchain license: {p.name}\n{p.read_text(encoding='utf-8')}")
(root / "RUST-COPYRIGHT-library.html").write_bytes((rust_doc / "COPYRIGHT-library.html").read_bytes())
(root / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
(root / "THIRD_PARTY_LICENSES.txt").write_text("\n".join(texts), encoding="utf-8")
if missing:
    raise SystemExit("Missing license files: " + ", ".join(missing))
print(f"Attributed {len(metadata['packages']) - 1} dependency packages; bundled license texts.")
