#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(windows)]
mod desktop;
fn main() {
    #[cfg(windows)]
    desktop::run();
    #[cfg(not(windows))]
    eprintln!("RunTerm est une application Windows. Utilisez npm run dev pour prévisualiser son interface.");
}
