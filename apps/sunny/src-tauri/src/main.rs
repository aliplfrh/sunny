// Sunny — launcher ala Raycast. Port GPL-3.0 dari vicinae ke Rust + Tauri.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Toggle window utama: terlihat → sembunyikan; tidak terlihat → tampilkan + fokus input.
/// (Perbaikan bug: dulu butuh fokus dulu untuk bisa menutup.)
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            // Memberi tampilan agar langsung meletakkan kursor di search bar
            let _ = app.emit("sunny:shown", ());
        }
    }
}

#[tauri::command]
fn ping() -> String {
    "pong".into()
}

/// Daftar semua aplikasi terinstal (dari Start Menu).
#[tauri::command]
fn list_apps() -> Vec<sun_core::apps::AppEntry> {
    sun_core::apps::scan_installed_apps()
}

/// Pencarian fuzzy terhadap aplikasi.
#[tauri::command]
fn search_apps(query: String) -> Vec<sun_core::search::Ranked<sun_core::apps::AppEntry>> {
    let apps = sun_core::apps::scan_installed_apps();
    sun_core::search::rank(&query, &apps)
}

/// Meluncurkan aplikasi lewat shortcut .lnk-nya.
#[tauri::command]
fn run_app(path: String) -> Result<(), String> {
    launching_app(&path)
}

/// Tutup (sembunyikan) jendela — dipanggil frontend (tombol Esc).
#[tauri::command]
fn hide_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(target_os = "windows")]
fn launching_app(path: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Membuka shortcut .lnk = meluncurkan aplikasi (konsep launch di vicinae).
    std::process::Command::new("cmd")
        .args(["/C", "start", "", path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "windows"))]
fn launching_app(path: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            ping,
            list_apps,
            search_apps,
            run_app,
            hide_main_window
        ])
        .setup(|app| {
            app.global_shortcut().register("Alt+Space")?;

            // Bug 2 yang juga dilaporkan: launcher harus bisa tutup tanpa
            // harus fokus dulu — klik di luar = sembunyikan (perilaku Raycast).
            if let Some(window) = app.get_webview_window("main") {
                let for_event = window.clone();
                window.on_window_event(move |e| {
                    if matches!(e, tauri::WindowEvent::Focused(false)) {
                        let _ = for_event.hide();
                    }
                });
            }

            // Pemanasan cache ikon di background: ekstrak ikon aplikasi sekali,
            // ke depannya scan loop berlangsung instan + ikon asli tampil.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let new_icons = sun_core::apps::build_icon_cache(sun_platform::extract_icon_png);
                if new_icons > 0 {
                    let _ = handle.emit("sunny:apps-updated", ());
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("gagal menjalankan Sunny");
}
