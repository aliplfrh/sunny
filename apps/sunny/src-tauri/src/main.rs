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
            show_or_focus_main_window(app);
        }
    }
}

/// Tampilkan + fokus (dipakai toggle & menu tray).
fn show_or_focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        // Memberi tampilan agar langsung meletakkan kursor di search bar
        let _ = app.emit("sunny:shown", ());
    }
}

use std::sync::Mutex;
use sun_core::clipboard::{ClipboardStore, ClipEntry};
use sun_core::settings::Settings;

/// Cache in-memory daftar aplikasi — menghindari disk IO per-tombol (perf fix).
pub struct AppState(pub Mutex<Vec<sun_core::apps::AppEntry>>);

/// Preferensi aktif — dimuat saat start, diubah dari panel Settings.
pub struct SettingsState(pub Mutex<Settings>);

/// Panel Settings sedang terbuka? Saat terbuka jendela dibuat lebih tinggi,
/// jadi mode Compact/Expanded baru diterapkan setelah panel ditutup.
pub struct SettingsUiOpen(pub std::sync::atomic::AtomicBool);

/// Terapkan opasitas native (layered window, tanpa transparansi WebView2).
fn apply_opacity(app: &tauri::AppHandle, percent: u32) {
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            sun_platform::set_window_opacity(hwnd.0 as isize, percent);
        }
    }
}

/// Ubah ukuran jendela sesuai mode (Compact/Expanded) lalu pusatkan kembali.
fn apply_window_mode(app: &tauri::AppHandle, settings: &Settings) {
    if app
        .state::<SettingsUiOpen>()
        .0
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return; // panel Settings sedang memakai jendela tinggi
    }
    if let Some(window) = app.get_webview_window("main") {
        let target_w = 720.0;
        let target_h = settings.window_height();
        let scale = window.scale_factor().unwrap_or(1.0);
        let changed = match window.inner_size() {
            Ok(s) => {
                (s.width as f64 / scale - target_w).abs() > 1.0
                    || (s.height as f64 / scale - target_h).abs() > 1.0
            }
            Err(_) => true,
        };
        if changed {
            let _ = window.set_size(tauri::LogicalSize::new(target_w, target_h));
            let _ = window.center();
        }
    }
}

/// Tampilkan/sembunyikan ikon tray tanpa membangun ulang.
fn apply_tray(app: &tauri::AppHandle, visible: bool) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_visible(visible);
    }
}

#[tauri::command]
fn ping() -> String {
    "pong".into()
}

/// Daftar semua aplikasi terinstal (sumber: cache; fallback scan bila masih kosong).
#[tauri::command]
fn list_apps(state: tauri::State<'_, AppState>) -> Vec<sun_core::apps::AppEntry> {
    let mut apps = state.0.lock().unwrap();
    if apps.is_empty() {
        *apps = sun_core::apps::scan_installed_apps();
    }
    apps.clone()
}

/// Pencarian fuzzy terhadap aplikasi (memakai cache, tanpa IO).
#[tauri::command]
fn search_apps(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Vec<sun_core::search::Ranked<sun_core::apps::AppEntry>> {
    let mut apps = state.0.lock().unwrap();
    if apps.is_empty() {
        *apps = sun_core::apps::scan_installed_apps();
    }
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

/// Hasil kalkulator (None bila kueri bukan ekspresi valid).
#[tauri::command]
fn calc(query: String) -> Option<String> {
    sun_core::calc::evaluate(&query)
}

/// Salin teks ke clipboard (fallback bila JS clipboard API gagal).
#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    match arboard::Clipboard::new() {
        Ok(mut cb) => cb
            .set_text(&text)
            .map_err(|e| format!("clipboard gagal: {e}")),
        Err(e) => Err(format!("clipboard tidak tersedia: {e}")),
    }
}

/// Seluruh riwayat clipboard (terbaru dulu) lengkap dengan metadata.
#[tauri::command]
fn clip_history(store: tauri::State<'_, ClipboardStore>) -> Vec<ClipEntry> {
    store.snapshot()
}

/// Status pause watcher clipboard (ala "Pause clipboard" Raycast).
#[tauri::command]
fn clip_pause_state(store: tauri::State<'_, ClipboardStore>) -> bool {
    store.is_paused()
}

/// Toggle pause/rekam clipboard. Kembalikan status terbaru.
#[tauri::command]
fn clip_toggle_pause(store: tauri::State<'_, ClipboardStore>) -> bool {
    store.toggle_pause()
}

/// Hapus satu entri riwayat berdasar id.
#[tauri::command]
fn clip_delete(id: u64, store: tauri::State<'_, ClipboardStore>) -> bool {
    let deleted = store.delete(id);
    if deleted {
        store.save();
    }
    deleted
}

/// Salin teks lalu tempelkan ke jendela aktif sebelumnya (ala Raycast):
/// set clipboard → sembunyikan launcher (fokus kembali ke app sebelumnya) →
/// kirim Ctrl+V. Best-effort: bila SendInput gagal, teks tetap tersalin.
#[tauri::command]
fn paste_to_active_window(app: tauri::AppHandle, text: String) -> Result<(), String> {
    {
        let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard: {e}"))?;
        cb.set_text(&text).map_err(|e| format!("clipboard: {e}"))?;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    // Beri waktu OS memindahkan fokus ke jendela sebelumnya sebelum Ctrl+V.
    std::thread::sleep(std::time::Duration::from_millis(220));
    let _ = sun_platform::send_paste();
    Ok(())
}

/// Dataset emoji penuh (dicache khusus backend; disaring di frontend).
#[tauri::command]
fn emoji_all() -> &'static [sun_core::emoji::EmojiItem] {
    sun_core::emoji::all()
}

/// Ukuran jendela saat panel Settings dibuka/ditutup — tidak mengubah setelan.
#[tauri::command]
fn settings_resize(app: tauri::AppHandle, open: bool) {
    use std::sync::atomic::Ordering;
    app.state::<SettingsUiOpen>().0.store(open, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window("main") {
        if open {
            let _ = window.set_size(tauri::LogicalSize::new(720.0, 700.0));
            let _ = window.center();
        } else {
            let settings = app.state::<SettingsState>().0.lock().unwrap().clone();
            apply_window_mode(&app, &settings);
        }
    }
}

/// Preferensi tersimpan (dipakai UI saat panel Settings dibuka).
#[tauri::command]
fn settings_get(state: tauri::State<'_, SettingsState>) -> Settings {
    state.0.lock().unwrap().clone()
}

/// Terapkan hanya efek visual (opacity + ukuran jendela) — dipakai saat
/// slider/stepper digeser, belum menyimpan ke disk.
#[tauri::command]
fn settings_preview(app: tauri::AppHandle, settings: Settings) {
    let mut s = settings;
    s.normalize();
    apply_opacity(&app, s.opacity);
    apply_window_mode(&app, &s);
}

/// Simpan + terapkan seluruh setelan (hotkey, tray, autostart, opacity, mode).
#[tauri::command]
fn settings_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
    settings: Settings,
) -> Result<Settings, String> {
    let mut next = settings;
    next.normalize();

    let current = state.0.lock().unwrap().clone();

    // Hotkey lebih dulu — bila gagal didaftarkan, jangan ubah setelan lain.
    if next.hotkey != current.hotkey {
        let gs = app.global_shortcut();
        let _ = gs.unregister(current.hotkey.as_str());
        if let Err(e) = gs.register(next.hotkey.as_str()) {
            let _ = gs.register(current.hotkey.as_str());
            return Err(format!("Hotkey cannot be registered: {e}"));
        }
    }

    if next.open_at_login != current.open_at_login {
        sun_platform::set_autostart(next.open_at_login)?;
    }

    apply_tray(&app, next.show_in_tray);
    apply_opacity(&app, next.opacity);
    apply_window_mode(&app, &next);
    next.save();

    *state.0.lock().unwrap() = next.clone();
    let _ = app.emit("sunny:settings-updated", &next);
    Ok(next)
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
        .manage(AppState(Mutex::new(Vec::new())))
        .manage(ClipboardStore::new())
        .manage(SettingsState(Mutex::new(Settings::load())))
        .manage(SettingsUiOpen(std::sync::atomic::AtomicBool::new(false)))
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
            hide_main_window,
            calc,
            copy_to_clipboard,
            clip_history,
            clip_pause_state,
            clip_toggle_pause,
            clip_delete,
            paste_to_active_window,
            emoji_all,
            settings_get,
            settings_preview,
            settings_update,
            settings_resize
        ])
        .setup(|app| {
            use tauri::menu::MenuBuilder;
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            // ── Setelan aktif (sudah dimuat sebelum window dibuat) ──
            let active = app.state::<SettingsState>().0.lock().unwrap().clone();

            // Hotkey tersimpan; fallback ke default bila gagal (mis. sudah dipakai).
            if app.global_shortcut().register(active.hotkey.as_str()).is_err() {
                let _ = app.global_shortcut().register("Alt+Space");
            }
            apply_opacity(app.handle(), active.opacity);
            apply_window_mode(app.handle(), &active);

            // ── Tray icon: klik kiri toggle, klik kanan menu ──
            // Ikon brand Sunny (dari logo pack) — dibaca langsung dari bytes
            // agar tray selalu memakai ikon resmi, bukan fallback bawaan.
            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                .ok()
                .or_else(|| app.default_window_icon().cloned());
            if let Some(icon) = tray_icon {
                let menu = MenuBuilder::new(app)
                    .text("show", "Show Sunny")
                    .text("settings", "Settings…")
                    .separator()
                    .text("quit", "Quit")
                    .build()?;

                let _tray = TrayIconBuilder::with_id("main-tray")
                    .icon(icon)
                    .tooltip("Sunny")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "show" => show_or_focus_main_window(app),
                        "settings" => {
                            show_or_focus_main_window(app);
                            let _ = app.emit("sunny:open-settings", ());
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if matches!(
                            event,
                            TrayIconEvent::Click {
                                button: MouseButton::Left,
                                button_state: MouseButtonState::Up,
                                ..
                            }
                        ) {
                            toggle_main_window(tray.app_handle());
                        }
                    })
                    .build(app)
                    .ok();
                apply_tray(app.handle(), active.show_in_tray);
            }

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

            // Pemanasan cache ikon + isi cache aplikasi di background:
            // ekstrak ikon sekali, ke depannya scan instan + ikon asli tampil.
            let handle = app.handle().clone();
            let state_handle = app.handle().clone();
            std::thread::spawn(move || {
                let new_icons = sun_core::apps::build_icon_cache(sun_platform::extract_icon_png);
                // Isi ulang cache aplikasi (kini ikon asli tersedia)
                let apps = sun_core::apps::scan_installed_apps();
                if let Some(st) = state_handle.try_state::<AppState>() {
                    *st.0.lock().unwrap() = apps;
                }
                if new_icons > 0 {
                    let _ = handle.emit("sunny:apps-updated", ());
                }
            });

            // ── Service clipboard history (Fase 2) ──
            // Load riwayat dari disk, lalu watcher polling ringan tiap 400ms.
            {
                use std::time::Duration;
                app.state::<ClipboardStore>().load();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let mut last_seq = sun_platform::clipboard_sequence();
                    loop {
                        std::thread::sleep(Duration::from_millis(400));
                        let seq = sun_platform::clipboard_sequence();
                        if seq == last_seq {
                            continue;
                        }
                        last_seq = seq;
                        // Pause aktif → jangan merekam (ala "Pause clipboard" Raycast)
                        let store = handle.state::<ClipboardStore>();
                        if store.is_paused() {
                            continue;
                        }
                        // Isi berubah — baca teksnya
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            if let Ok(text) = cb.get_text() {
                                if store.push(&text) {
                                    store.save();
                                    let _ = handle.emit("sunny:clip-updated", ());
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("gagal menjalankan Sunny");
}
