#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod benchmark;
#[cfg(windows)]
mod ui;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args.iter().position(|s| s == "--benchmark-core") {
        if let Some(path) = args.get(i + 1) {
            benchmark::run(std::path::Path::new(path));
        }
        return;
    }
    #[cfg(windows)]
    ui::run();
    #[cfg(not(windows))]
    eprintln!("VoiceStage GUI requires Windows 11 x64. Core tests run cross-platform.");
}
