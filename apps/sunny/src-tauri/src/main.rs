// Sunny — launcher ala Raycast. Port GPL-3.0 dari vicinae ke Rust + Tauri.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Toggle window utama (show+focus / hide) — dipanggil dari hotkey global.
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
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
        .invoke_handler(tauri::generate_handler![ping, list_apps, search_apps, run_app])
        .setup(|app| {
            app.global_shortcut().register("Alt+Space")?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("gagal menjalankan Sunny");
}
