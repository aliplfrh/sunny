// Sunny — launcher ala Raycast. Port GPL-3.0 dari vicinae ke Rust + Tauri.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

mod presentation;
mod version;

/// Toggle window utama: terlihat → sembunyikan; tidak terlihat → tampilkan + fokus input.
/// (Perbaikan bug: dulu butuh fokus dulu untuk bisa menutup.)
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) || app.state::<WindowShows>().launcher.is_pending()
        {
            app.state::<WindowShows>().launcher.cancel();
            let _ = window.hide();
        } else {
            show_or_focus_main_window(app);
        }
    }
}

/// Tampilkan + fokus (dipakai toggle & menu tray).
/// Snapshot desktop diambil SELAGI jendela tersembunyi, sebelum `show`,
/// sehingga yang tertangkap adalah isi layar di belakang launcher.
fn show_or_focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.set_focus();
        return;
    }
    let Some(ticket) = app.state::<WindowShows>().launcher.begin() else {
        return;
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let backdrop = capture_window_backdrop(&handle, "main");
        let target = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if !target.state::<WindowShows>().launcher.finish(ticket) {
                return;
            }
            *target.state::<BackdropState>().0.lock().unwrap() = backdrop.clone();
            let _ = target.emit_to("main", "sunny:backdrop", backdrop);
            if let Some(window) = target.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = target.emit_to("main", "sunny:shown", ());
            }
        });
    });
}

use std::sync::Mutex;
use sun_core::clipboard::{ClipEntry, ClipboardStore};
use sun_core::settings::Settings;

/// Cache in-memory daftar aplikasi — menghindari disk IO per-tombol (perf fix).
pub struct AppState(pub Mutex<Vec<sun_core::apps::AppEntry>>);

/// Serializes the first full scan; other cold requests wait for its result instead
/// of each rebuilding the Start Menu + Microsoft Store index concurrently.
/// `AppState` stays empty only while a scan is in progress, so this is the
/// initialization lock, not a hot-path lock.
pub struct AppIndexInit(pub Mutex<()>);

/// Preferensi aktif — dimuat saat start, diubah dari panel Settings.
pub struct SettingsState(pub Mutex<Settings>);
pub struct SettingsTransactions(pub Mutex<()>);

/// RAII ownership: replacing this value removes the passive keyboard observer.
pub struct WinSpaceState(pub Mutex<Option<sun_platform::WinSpaceObserver>>);

#[derive(Default)]
pub struct WindowShows {
    launcher: presentation::ShowRequest,
    settings: presentation::ShowRequest,
}

pub struct LauncherState(pub Mutex<sun_core::launcher::LauncherHistory>);

/// Panel Settings kini jendela terpisah; penanda ini dipertahankan agar
/// mode Compact/Expanded tidak diterapkan saat setelan dibuka.
pub struct SettingsUiOpen(pub std::sync::atomic::AtomicBool);

/// Snapshot desktop di belakang launcher (data URL PNG) untuk backdrop CSS.
pub struct BackdropState(pub Mutex<Option<String>>);

/// Snapshot desktop di belakang jendela Settings (terpisah dari launcher,
/// karena area tangkapnya berbeda).
pub struct SettingsBackdropState(pub Mutex<Option<String>>);

/// Indeks file pengguna + status pembangunan di background.
pub struct FileIndexState {
    pub files: Mutex<Vec<sun_core::files::FileEntry>>,
    pub ready: std::sync::atomic::AtomicBool,
}

/// Ambil snapshot layar di posisi jendela (harus dipanggil saat jendela
/// tersembunyi). Hasilnya dipakai CSS sebagai background launcher sehingga
/// opacity hanya memudarkan background — teks dan ikon tetap pekat.
fn capture_window_backdrop(app: &tauri::AppHandle, label: &str) -> Option<String> {
    let window = app.get_webview_window(label)?;
    if window.is_visible().unwrap_or(true) {
        return None;
    }
    sun_platform::flush_desktop_compositor();
    // Tauri's client geometry and GDI coordinates are already physical pixels.
    // Using outer bounds bakes invisible borders into the CSS background and
    // shifts/stretches it relative to the WebView, especially on scaled displays.
    let pos = window.inner_position().ok()?;
    let size = window.inner_size().ok()?;
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let width = i32::try_from(size.width).ok()?;
    let height = i32::try_from(size.height).ok()?;
    let png = sun_platform::capture_screen_png(pos.x, pos.y, width, height)?;
    use base64::Engine;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
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

#[tauri::command]
fn app_version(app: tauri::AppHandle) -> version::VersionInfo {
    version::VersionInfo::current(&app.package_info().version.to_string())
}

/// Indeks aplikasi lengkap: shortcut Start Menu + aplikasi Microsoft Store.
///
/// Lapisan platform membaca manifest AppX dan menyediakan isi logo; konversi ke
/// tipe netral (`StoreAppInput`) terjadi di sini supaya `sun-core` tetap tidak
/// bergantung pada `sun-platform`.
fn load_app_index() -> Vec<sun_core::apps::AppEntry> {
    let shortcuts = sun_core::apps::scan_installed_apps();
    let store = sun_platform::list_store_apps()
        .into_iter()
        .map(|app| sun_core::apps::StoreAppInput {
            icon_png: sun_platform::store_app_icon_png(app.icon_path.as_deref()),
            name: app.name,
            app_id: app.app_id,
        })
        .collect();
    sun_core::apps::merge_store_apps(shortcuts, store)
}

/// Run disk/OS work on a standard background thread without blocking the command loop.
async fn background<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (send, mut receive) = tauri::async_runtime::channel(1);
    std::thread::Builder::new()
        .name("sunny-command".into())
        .spawn(move || {
            let _ = send.blocking_send(work());
        })
        .map_err(|e| format!("Cannot start background task: {e}"))?;
    receive
        .recv()
        .await
        .ok_or_else(|| "Background task stopped unexpectedly".to_string())?
}

fn app_index(app: &tauri::AppHandle) -> Result<Vec<sun_core::apps::AppEntry>, String> {
    let state = app.state::<AppState>();
    if let Ok(current) = state.0.lock() {
        if !current.is_empty() {
            return Ok(current.clone());
        }
    }
    // One initializer at a time. Re-check after waiting: the previous holder may
    // already have populated the cache, including on the background warm-up thread.
    let init = app.state::<AppIndexInit>();
    let _init_guard = init.0.lock().map_err(|_| "Application index unavailable")?;
    let mut cache = state
        .0
        .lock()
        .map_err(|_| "Application index unavailable")?;
    if cache.is_empty() {
        *cache = load_app_index();
    }
    Ok(cache.clone())
}

/// Application cache, with initial scanning off the UI thread.
#[tauri::command]
async fn list_apps(app: tauri::AppHandle) -> Result<Vec<sun_core::apps::AppEntry>, String> {
    background(move || app_index(&app)).await
}

/// Pencarian fuzzy terhadap aplikasi (memakai cache, tanpa IO).
#[tauri::command]
async fn search_apps(
    query: String,
    app: tauri::AppHandle,
) -> Result<Vec<sun_core::search::Ranked<sun_core::apps::AppEntry>>, String> {
    background(move || Ok(sun_core::search::rank(&query, &app_index(&app)?))).await
}

#[tauri::command]
async fn launcher_home(app: tauri::AppHandle) -> Result<sun_core::launcher::LauncherHome, String> {
    background(move || {
        let apps = app_index(&app)?;
        let state = app.state::<LauncherState>();
        let history = state.0.lock().map_err(|_| "Launcher history unavailable")?;
        Ok(history.home(&apps))
    })
    .await
}

#[tauri::command]
async fn app_set_favorite(
    app: tauri::AppHandle,
    path: String,
    favorite: bool,
) -> Result<sun_core::launcher::LauncherHome, String> {
    background(move || {
        let apps = app_index(&app)?;
        if !apps.iter().any(|entry| entry.path == path) {
            return Err("This application is no longer in the app index".into());
        }
        let state = app.state::<LauncherState>();
        let mut history = state.0.lock().map_err(|_| "Launcher history unavailable")?;
        history.set_favorite(&path, favorite)?;
        Ok(history.home(&apps))
    })
    .await
}

/// Count only a successful launch of an indexed application, never a file-search open.
#[tauri::command]
async fn run_app(app: tauri::AppHandle, path: String) -> Result<(), String> {
    background(move || {
        let apps = app_index(&app)?;
        if !apps.iter().any(|entry| entry.path == path) {
            return Err("This application is no longer in the app index".into());
        }
        launching_app(&path)?;
        // The application is already open; a history write failure must not be
        // reported as a launch failure (that would invite a duplicate launch).
        if let Ok(state) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.state::<LauncherState>()
        })) {
            if let Ok(mut history) = state.0.lock() {
                if let Err(error) = history.record_launch(&path) {
                    let _ = app.emit_to(
                        "main",
                        "sunny:launcher-warning",
                        format!("Application opened, but its usage could not be saved: {error}"),
                    );
                }
            }
        }
        Ok(())
    })
    .await
}

/// Tutup (sembunyikan) jendela — dipanggil frontend (tombol Esc).
#[tauri::command]
fn hide_main_window(app: tauri::AppHandle) {
    app.state::<WindowShows>().launcher.cancel();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// Hasil kalkulator (None bila kueri bukan ekspresi valid).
#[tauri::command]
fn calc(query: String) -> Option<String> {
    sun_core::calc::evaluate(&query)
}

/// Lorem ipsum text (None bila kueri bukan permintaan lorem).
#[tauri::command]
fn lorem(query: String) -> Option<String> {
    sun_core::lorem::evaluate(&query)
}

/// Mesin pencari yang tersedia untuk Quick Web Search.
#[tauri::command]
fn search_engines() -> Vec<sun_core::websearch::SearchEngine> {
    sun_core::websearch::default_engines()
}

/// Buka hasil Quick Web Search di browser default.
///
/// Sunny sendiri tidak melakukan permintaan jaringan: URL dirender di sini lalu
/// dibuka lewat handler default Windows, jadi mesin pencari yang dihubungi.
#[tauri::command]
fn web_search(query: String, engine_id: String) -> Result<String, String> {
    let engine = sun_core::websearch::engine_by_id(&engine_id);
    let (_, url) = sun_core::websearch::prepare(&query, &engine).map_err(|reason| match reason {
        sun_core::websearch::NotASearch::MissingQuestionMark => {
            "Add ? to the end of the query to search the web.".to_string()
        }
        sun_core::websearch::NotASearch::EmptyQuery => "Type something to search for.".to_string(),
        sun_core::websearch::NotASearch::TooShort => {
            "Type at least two characters to search for.".to_string()
        }
    })?;
    sun_platform::shell_open(&url)?;
    Ok(url)
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

/// Snapshot desktop terakhir (data URL PNG) untuk backdrop launcher.
#[tauri::command]
fn backdrop_get(state: tauri::State<'_, BackdropState>) -> Option<String> {
    state.0.lock().unwrap().clone()
}

/// Backdrop khusus jendela Settings (area tangkapnya berbeda dari launcher).
#[tauri::command]
fn settings_backdrop_get(state: tauri::State<'_, SettingsBackdropState>) -> Option<String> {
    state.0.lock().unwrap().clone()
}

/// Cari file/folder di indeks pengguna (fuzzy, seperti "Search Files" Raycast).
#[tauri::command]
fn search_files(
    query: String,
    state: tauri::State<'_, FileIndexState>,
) -> sun_core::files::FileSearchResult {
    use std::sync::atomic::Ordering;
    let ready = state.ready.load(Ordering::Relaxed);
    let files = state.files.lock().unwrap();
    let total = files.len();
    let items = sun_core::files::search(&files, &query, 60);
    sun_core::files::FileSearchResult {
        items,
        ready,
        total,
    }
}

/// Bangun ulang indeks file di background (dipakai tombol "Rebuild").
#[tauri::command]
fn rebuild_file_index(app: tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    if let Some(st) = app.try_state::<FileIndexState>() {
        st.ready.store(false, Ordering::Relaxed);
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let files = sun_core::files::build_index();
        if let Some(st) = handle.try_state::<FileIndexState>() {
            *st.files.lock().unwrap() = files;
            st.ready.store(true, Ordering::Relaxed);
        }
        let _ = handle.emit("sunny:files-updated", ());
    });
}

/// Buka file dengan aplikasi default (ShellExecuteW; aman untuk semua nama file).
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    sun_platform::shell_open(&path)
}

/// Tampilkan file terpilih di File Explorer (ala "Show in Finder").
#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // Perhatikan: /select, dan path harus satu argumen (boleh ada spasi).
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{path}"))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path);
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Buka jendela Settings terpisah (ala Raycast) dan sembunyikan launcher.
#[tauri::command]
fn settings_open(app: tauri::AppHandle) {
    let Some(settings) = app.get_webview_window("settings") else {
        return;
    };
    app.state::<WindowShows>().launcher.cancel();
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    // Never capture Settings into its own backdrop when it is already open.
    if settings.is_visible().unwrap_or(false) {
        let _ = settings.unminimize();
        let _ = settings.set_focus();
        return;
    }
    let Some(ticket) = app.state::<WindowShows>().settings.begin() else {
        return;
    };
    std::thread::spawn(move || {
        let backdrop = capture_window_backdrop(&app, "settings");
        let target = app.clone();
        let _ = app.run_on_main_thread(move || {
            if !target.state::<WindowShows>().settings.finish(ticket) {
                return;
            }
            *target.state::<SettingsBackdropState>().0.lock().unwrap() = backdrop.clone();
            let _ = target.emit_to("settings", "sunny:settings-backdrop", backdrop);
            if let Some(win) = target.get_webview_window("settings") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        });
    });
}

/// Tutup jendela Settings (dipanggil dari tombol Esc / back).
#[tauri::command]
fn settings_close(app: tauri::AppHandle) {
    app.state::<WindowShows>().settings.cancel();
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.hide();
    }
}

// ── Auto-update ────────────────────────────────────────────────────────────
// Memakai tauri-plugin-updater. Endpoint & public key ada di tauri.conf.json;
// artefak rilis ditandatangani dengan kunci privat yang sama.

/// Ringkasan rilis baru yang dikirim ke UI.
#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    current_version: String,
    body: Option<String>,
    date: Option<String>,
}

/// Cek apakah ada versi lebih baru di endpoint updater.
#[tauri::command]
async fn update_check(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    Ok(found.map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        body: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
    }))
}

/// Unduh + pasang pembaruan, lalu keluar agar installer bisa mengganti biner.
/// Progres dikirim ke UI lewat event `sunny:update-progress`.
#[tauri::command]
async fn update_install(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Err("No update available".into());
    };

    let handle = app.clone();
    let mut downloaded: u64 = 0;
    let mut total: u64 = 0;

    update
        .download_and_install(
            move |chunk, content_length| {
                if let Some(len) = content_length {
                    total = len;
                }
                downloaded += chunk as u64;
                let pct = if total > 0 {
                    (downloaded * 100 / total).min(100) as u8
                } else {
                    0
                };
                let _ = handle.emit("sunny:update-progress", pct);
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    // Di Windows installer sudah dijalankan dan proses ini akan ditutup.
    Ok(())
}

/// Preferensi tersimpan (dipakai UI saat panel Settings dibuka).
#[tauri::command]
fn settings_get(state: tauri::State<'_, SettingsState>) -> Settings {
    state.0.lock().unwrap().clone()
}

/// Kembalikan semua setelan ke default pabrik lalu simpan.
#[tauri::command]
async fn settings_reset(app: tauri::AppHandle) -> Result<Settings, String> {
    background(move || persist_settings(&app, Settings::default())).await
}

/// Lokasi folder setelan (untuk tombol "Open settings folder").
#[tauri::command]
fn settings_dir() -> String {
    let base = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
    format!("{base}\\Sunny")
}

/// Terapkan hanya efek visual (ukuran jendela) — dipakai saat slider/stepper
/// digeser, belum menyimpan ke disk. Opacity kini murni CSS (background-only),
/// jadi UI langsung mengubah `--bg-tint` tanpa perlu round-trip backend.
#[tauri::command]
fn settings_preview(app: tauri::AppHandle, settings: Settings) {
    let mut s = settings;
    s.normalize();
    apply_window_mode(&app, &s);
}

/// Win+Space is observed, never intercepted; Windows keeps its language shortcut.
fn register_hotkey(app: &tauri::AppHandle, accel: &str) -> Result<(), String> {
    if sun_platform::is_win_space(accel) {
        let state = app.state::<WinSpaceState>();
        let mut observer = state.0.lock().unwrap();
        if observer.is_none() {
            let handle = app.clone();
            *observer = Some(sun_platform::WinSpaceObserver::start(move || {
                let target = handle.clone();
                // Queue window work only after the physical chord is fully released.
                let _ = handle.run_on_main_thread(move || {
                    // Ignore queued notifications after changing to another shortcut.
                    let active = target
                        .state::<SettingsState>()
                        .0
                        .lock()
                        .unwrap()
                        .hotkey
                        .clone();
                    if sun_platform::is_win_space(&active) {
                        toggle_main_window(&target);
                    }
                });
            })?);
        }
        return Ok(());
    }
    let gs = app.global_shortcut();
    match gs.register(accel) {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            let lowered = msg.to_ascii_lowercase();
            if !(lowered.contains("already registered") || msg.contains("1409")) {
                return Err(format!("Hotkey cannot be registered: {msg}"));
            }
            Err(format!(
                "{accel} is reserved by Windows or used by another application. Pick a different shortcut."
            ))
        }
    }
}

/// Register the replacement first, preserving the working shortcut on failure.
fn replace_hotkey(app: &tauri::AppHandle, previous: &str, next: &str) -> Result<(), String> {
    if previous == next
        || (sun_platform::is_win_space(previous) && sun_platform::is_win_space(next))
    {
        return Ok(());
    }
    // Accelerator strings can differ in case/alias while referring to the same key.
    if !sun_platform::is_win_space(previous) && !sun_platform::is_win_space(next) {
        let old = previous
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|e| e.to_string())?;
        let new = next
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|e| e.to_string())?;
        if old == new {
            return Ok(());
        }
    }
    register_hotkey(app, next)?;
    if sun_platform::is_win_space(previous) {
        let old = app
            .state::<WinSpaceState>()
            .0
            .lock()
            .ok()
            .and_then(|mut s| s.take());
        drop(old);
    } else if let Err(error) = app.global_shortcut().unregister(previous) {
        // Do not leave two bindings active when unregistering the previous one fails.
        if sun_platform::is_win_space(next) {
            let old = app
                .state::<WinSpaceState>()
                .0
                .lock()
                .ok()
                .and_then(|mut s| s.take());
            drop(old);
        } else {
            let _ = app.global_shortcut().unregister(next);
        }
        return Err(format!("Cannot replace shortcut: {error}"));
    }
    Ok(())
}

/// Undo a hotkey change, reporting when the old shortcut could not be restored.
fn restore_hotkey(app: &tauri::AppHandle, previous: &str, applied: &str, error: String) -> String {
    match replace_hotkey(app, applied, previous) {
        Ok(()) => error,
        Err(rollback) => {
            format!("{error} Your previous shortcut ({previous}) could not be restored: {rollback}")
        }
    }
}

/// Simpan + terapkan seluruh setelan (hotkey, tray, autostart, opacity, mode).
#[tauri::command]
async fn settings_update(app: tauri::AppHandle, settings: Settings) -> Result<Settings, String> {
    background(move || persist_settings(&app, settings)).await
}

fn persist_settings(app: &tauri::AppHandle, mut next: Settings) -> Result<Settings, String> {
    // Serialize complete transactions across both windows, without holding the
    // SettingsState lock while native window operations are marshalled to the UI.
    let transactions = app.state::<SettingsTransactions>();
    let _transaction = transactions
        .0
        .lock()
        .map_err(|_| "Settings save unavailable")?;
    let state = app.state::<SettingsState>();
    let current = state.0.lock().map_err(|_| "Settings unavailable")?.clone();
    next.normalize();
    replace_hotkey(app, &current.hotkey, &next.hotkey)?;
    if next.open_at_login != current.open_at_login {
        if let Err(error) = sun_platform::set_autostart(next.open_at_login) {
            let error = restore_hotkey(app, &current.hotkey, &next.hotkey, error);
            apply_window_mode(app, &current);
            return Err(error);
        }
    }
    if let Err(error) = next.save() {
        // Restore native state before reporting; a failed rollback is surfaced too.
        let mut error = error;
        if next.open_at_login != current.open_at_login {
            if let Err(rollback) = sun_platform::set_autostart(current.open_at_login) {
                error = format!("{error} Open at Login could not be restored: {rollback}");
            }
        }
        let error = restore_hotkey(app, &current.hotkey, &next.hotkey, error);
        apply_window_mode(app, &current);
        return Err(error);
    }
    *state.0.lock().map_err(|_| "Settings unavailable")? = next.clone();
    apply_tray(app, next.show_in_tray);
    apply_window_mode(app, &next);
    let _ = app.emit("sunny:settings-updated", &next);
    Ok(next)
}

/// Luncurkan aplikasi.
///
/// - Shortcut `.lnk` / path biasa → `ShellExecuteW` (handler default Windows).
/// - AppUserModelID (`Publisher.Nama_cv...!App`) → activation manager Microsoft
///   Store. Bentuknya khas: ada `!` dan tidak ada pemisah path, jadi tidak
///   mungkin tertukar dengan path file.
fn launching_app(path: &str) -> Result<(), String> {
    if path.contains('!') && !path.contains('\\') && !path.contains('/') {
        return sun_platform::launch_store_app(path);
    }
    sun_platform::shell_open(path)
}

fn is_autostart_launch(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| argument == "--autostart")
}

fn main() {
    tauri::Builder::default()
        // Single-instance HARUS plugin pertama: mencegah dua proses menulis
        // settings.json bersamaan (penyebab setelan tampak "tidak tersimpan").
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !is_autostart_launch(&argv) {
                show_or_focus_main_window(app);
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState(Mutex::new(Vec::new())))
        .manage(AppIndexInit(Mutex::new(())))
        .manage(ClipboardStore::new())
        .manage(SettingsState(Mutex::new(Settings::load())))
        .manage(SettingsTransactions(Mutex::new(())))
        .manage(LauncherState(Mutex::new(
            sun_core::launcher::LauncherHistory::load(),
        )))
        .manage(WinSpaceState(Mutex::new(None)))
        .manage(WindowShows::default())
        .manage(SettingsUiOpen(std::sync::atomic::AtomicBool::new(false)))
        .manage(BackdropState(Mutex::new(None)))
        .manage(SettingsBackdropState(Mutex::new(None)))
        .manage(FileIndexState {
            files: Mutex::new(Vec::new()),
            ready: std::sync::atomic::AtomicBool::new(false),
        })
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
            app_version,
            list_apps,
            launcher_home,
            app_set_favorite,
            search_apps,
            run_app,
            hide_main_window,
            calc,
            lorem,
            search_engines,
            web_search,
            copy_to_clipboard,
            clip_history,
            clip_pause_state,
            clip_toggle_pause,
            clip_delete,
            paste_to_active_window,
            emoji_all,
            backdrop_get,
            settings_backdrop_get,
            search_files,
            rebuild_file_index,
            open_path,
            reveal_path,
            settings_get,
            settings_reset,
            settings_dir,
            settings_preview,
            settings_update,
            settings_open,
            settings_close,
            update_check,
            update_install
        ])
        .setup(|app| {
            use tauri::menu::MenuBuilder;
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            // ── Setelan aktif (sudah dimuat sebelum window dibuat) ──
            let active = app.state::<SettingsState>().0.lock().unwrap().clone();

            // Saved intent alone does not prove the Run entry exists. Repair a missing
            // entry or old executable path after install/update on every application start.
            if let Err(error) = sun_platform::set_autostart(active.open_at_login) {
                eprintln!("Sunny startup registration: {error}");
            }

            // Hotkey tersimpan; fallback ke default bila gagal (mis. sudah dipakai).
            if register_hotkey(app.handle(), &active.hotkey).is_err() {
                register_hotkey(app.handle(), "Alt+Space").map_err(std::io::Error::other)?;
                // Keep the in-memory value aligned with the binding that actually works.
                app.state::<SettingsState>().0.lock().unwrap().hotkey = "Alt+Space".into();
            }
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
                        for_event
                            .app_handle()
                            .state::<WindowShows>()
                            .launcher
                            .cancel();
                        let _ = for_event.hide();
                    }
                });
            }

            // Jendela Settings: tombol X hanya menyembunyikan (bukan menutup),
            // supaya bisa dibuka lagi tanpa membuat ulang WebView.
            if let Some(win) = app.get_webview_window("settings") {
                let for_event = win.clone();
                win.on_window_event(move |e| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                        api.prevent_close();
                        for_event
                            .app_handle()
                            .state::<WindowShows>()
                            .settings
                            .cancel();
                        let _ = for_event.hide();
                    }
                });
            }

            // Pemanasan cache ikon + isi cache aplikasi di background:
            // ekstrak ikon sekali, ke depannya scan instan + ikon asli tampil.
            let handle = app.handle().clone();
            let state_handle = app.handle().clone();
            std::thread::spawn(move || {
                sun_core::apps::build_icon_cache(sun_platform::extract_icon_png);
                // Isi ulang cache aplikasi (kini ikon asli tersedia). Ambil lock inisialisasi
                // yang sama agar cold-start request tidak memicu scan kedua secara bersamaan.
                let apps = load_app_index();
                if let Ok(init) = state_handle.state::<AppIndexInit>().0.lock() {
                    if let Some(st) = state_handle.try_state::<AppState>() {
                        if let Ok(mut cache) = st.0.lock() {
                            *cache = apps;
                        }
                    }
                    drop(init);
                }
                let _ = handle.emit("sunny:apps-updated", ());
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

            // ── Indeks file pengguna (Fase 2: "Search Files") ──
            // Dibangun sekali di background; command `search_files` hanya
            // membaca indeks di memori (tanpa disk IO per ketikan).
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let files = sun_core::files::build_index();
                    if let Some(st) = handle.try_state::<FileIndexState>() {
                        *st.files.lock().unwrap() = files;
                        st.ready.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    let _ = handle.emit("sunny:files-updated", ());
                });
            }

            // Jendela memakai `visible: false` di config agar snapshot desktop
            // bisa diambil sebelum tampil. Tampilkan sekarang (sekalian capture).
            if !is_autostart_launch(&std::env::args().collect::<Vec<_>>()) {
                show_or_focus_main_window(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("gagal menjalankan Sunny")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let observer = app.state::<WinSpaceState>().0.lock().unwrap().take();
                drop(observer);
            }
        });
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn login_start_is_background_only_and_explicit() {
        assert!(is_autostart_launch(&[
            "sunny.exe".into(),
            "--autostart".into()
        ]));
        assert!(!is_autostart_launch(&["sunny.exe".into()]));
        assert!(!is_autostart_launch(&[
            "sunny.exe".into(),
            "--not-autostart".into()
        ]));
    }

    #[test]
    fn background_commands_forward_success_and_failure() {
        assert_eq!(
            tauri::async_runtime::block_on(background(|| Ok(42))),
            Ok(42)
        );
        assert_eq!(
            tauri::async_runtime::block_on(background::<()>(|| Err("Disk unavailable".into()))),
            Err("Disk unavailable".into())
        );
    }
}
