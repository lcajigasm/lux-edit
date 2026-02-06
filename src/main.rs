mod app;
mod editor;
mod lsp;
mod plugin;
mod syntax;
mod ui;

use app::LuxApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // Reduce noisy macOS system logs in the terminal.
    std::env::set_var("OS_ACTIVITY_MODE", "disable");
    #[cfg(target_os = "macos")]
    {
        silence_stderr();
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Lux Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "Lux Editor",
        options,
        Box::new(|cc| Ok(Box::new(LuxApp::new(cc)))),
    )
}

#[cfg(target_os = "macos")]
fn silence_stderr() {
    unsafe {
        use std::os::unix::io::AsRawFd;
        if let Ok(devnull) = std::fs::OpenOptions::new().write(true).open("/dev/null") {
            libc::dup2(devnull.as_raw_fd(), libc::STDERR_FILENO);
        }
    }
}
