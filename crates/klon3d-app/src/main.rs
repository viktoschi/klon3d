//! `klon3d-app [muster.sm2d]` – opens the application window, optionally with a pattern.

use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use klon3d_app::App;

const AUFRUF: &str = "Aufruf: klon3d-app [muster.sm2d]";

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let datei = args.next().map(PathBuf::from);
    if args.next().is_some() {
        eprintln!("zu viele Argumente\n{AUFRUF}");
        return ExitCode::FAILURE;
    }
    if let Some(d) = &datei {
        if matches!(d.to_str(), Some("-h" | "--help" | "/?")) {
            println!("{AUFRUF}");
            return ExitCode::SUCCESS;
        }
    }

    let optionen = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("klon3d")
            .with_app_id("klon3d")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    let ergebnis = eframe::run_native(
        "klon3d",
        optionen,
        Box::new(move |cc| {
            let mut app = App::neu(cc);
            if let Some(d) = datei {
                // Errors end up in the status bar and the error window.
                app.muster_laden(d);
            }
            Ok(Box::new(app))
        }),
    );
    match ergebnis {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("klon3d-app: Fenster lässt sich nicht öffnen: {e}");
            ExitCode::FAILURE
        }
    }
}
