// Sunny — launcher ala Raycast. Port GPL-3.0 dari vicinae ke Rust + Tauri.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

mod index;
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

use std::sync::{Arc, Mutex};
use sun_core::clipboard::{ClipEntry, ClipboardStore};
use sun_core::settings::Settings;

/// Cache in-memory daftar aplikasi — menghindari disk IO per-tombol (perf fix).
pub struct AppState(pub index::SnapshotIndex<sun_core::apps::AppEntry>);

/// Preferensi aktif — dimuat saat start, diubah dari panel Settings.
pub struct SettingsState(pub Mutex<Settings>);
pub struct SettingsTransactions(pub Mutex<()>);

#[derive(Default)]
pub struct OmnigetSetupState {
    busy: std::sync::atomic::AtomicBool,
    gate: Mutex<()>,
}

impl OmnigetSetupState {
    fn reserve<'a>(
        &'a self,
        downloads: &sun_core::video::Downloads,
    ) -> Result<OmnigetSetupGuard<'a>, String> {
        use std::sync::atomic::Ordering;
        let _gate = self
            .gate
            .lock()
            .map_err(|_| "OmniGet setup state unavailable.")?;
        self.busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| "OmniGet setup is already running.")?;
        let guard = OmnigetSetupGuard(&self.busy);
        if downloads.status()?.phase == sun_core::video::Phase::Running {
            return Err(
                "Cancel or finish the active video download before setting up OmniGet.".into(),
            );
        }
        Ok(guard)
    }

    fn begin_download(
        &self,
        downloads: &sun_core::video::Downloads,
    ) -> Result<(sun_core::video::Status, Arc<std::sync::atomic::AtomicBool>), String> {
        let _gate = self
            .gate
            .lock()
            .map_err(|_| "OmniGet setup state unavailable.")?;
        if self.busy.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Wait for OmniGet setup to finish before starting a download.".into());
        }
        downloads.begin()
    }
}

struct OmnigetSetupGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for OmnigetSetupGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

/// RAII ownership: replacing this value removes the passive keyboard observer.
pub struct WinSpaceState(pub Mutex<Option<sun_platform::WinSpaceObserver>>);

#[derive(Default)]
pub struct WindowShows {
    launcher: presentation::ShowRequest,
    settings: presentation::ShowRequest,
}

pub struct LauncherState(pub Mutex<sun_core::launcher::LauncherHistory>);

/// Snapshot desktop di belakang launcher (data URL PNG) untuk backdrop CSS.
pub struct BackdropState(pub Mutex<Option<String>>);

/// Snapshot desktop di belakang jendela Settings (terpisah dari launcher,
/// karena area tangkapnya berbeda).
pub struct SettingsBackdropState(pub Mutex<Option<String>>);

/// Indeks file pengguna + status pembangunan di background.
pub struct FileIndexState {
    pub files: Mutex<Arc<Vec<sun_core::files::FileEntry>>>,
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
            // A panic would otherwise drop the sender and leave the UI waiting
            // forever without an answer, so turn it into a visible error.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|panic| {
                    let detail = panic
                        .downcast_ref::<&str>()
                        .map(|text| (*text).to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    eprintln!("Sunny background task panicked: {detail}");
                    Err("The task failed unexpectedly. Nothing was changed.".to_string())
                });
            let _ = send.blocking_send(result);
        })
        .map_err(|e| format!("Cannot start background task: {e}"))?;
    receive
        .recv()
        .await
        .ok_or_else(|| "Background task stopped unexpectedly".to_string())?
}

fn app_index(app: &tauri::AppHandle) -> Result<Arc<Vec<sun_core::apps::AppEntry>>, String> {
    app.state::<AppState>().0.get_or_init(load_app_index)
}

/// Application cache, with initial scanning off the UI thread.
#[tauri::command]
async fn list_apps(app: tauri::AppHandle) -> Result<Vec<sun_core::apps::AppEntry>, String> {
    background(move || Ok(app_index(&app)?.as_ref().clone())).await
}

/// Pencarian fuzzy terhadap aplikasi (memakai cache, tanpa IO).
#[tauri::command]
async fn search_apps(
    query: String,
    app: tauri::AppHandle,
) -> Result<Vec<sun_core::search::Ranked<sun_core::apps::AppEntry>>, String> {
    background(move || {
        let mut hits = sun_core::search::rank(&query, &app_index(&app)?);
        // Icons dominate the payload; only the best matches are ever rendered.
        hits.truncate(60);
        Ok(hits)
    })
    .await
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
        if let Ok(mut history) = app.state::<LauncherState>().0.lock() {
            if let Err(error) = history.record_launch(&path) {
                let _ = app.emit_to(
                    "main",
                    "sunny:launcher-warning",
                    format!("Application opened, but its usage could not be saved: {error}"),
                );
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
async fn web_search(query: String, engine_id: String) -> Result<String, String> {
    let engine = sun_core::websearch::engine_by_id(&engine_id);
    let (_, url) =
        sun_core::websearch::prepare(&query, &engine).map_err(|reason| match reason {
            sun_core::websearch::NotASearch::MissingQuestionMark => {
                "Add ? to the end of the query to search the web.".to_string()
            }
            sun_core::websearch::NotASearch::EmptyQuery => {
                "Type something to search for.".to_string()
            }
            sun_core::websearch::NotASearch::TooShort => {
                "Type at least two characters to search for.".to_string()
            }
        })?;
    background(move || {
        sun_platform::shell_open(&url)?;
        Ok(url)
    })
    .await
}

/// Copy text to the clipboard (fallback when the WebView clipboard API fails).
#[tauri::command]
async fn copy_to_clipboard(text: String) -> Result<(), String> {
    background(move || {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
        clipboard
            .set_text(&text)
            .map_err(|e| format!("Could not copy to the clipboard: {e}"))
    })
    .await
}

/// Generate a QR matrix locally. No URL or input leaves Sunny.
#[tauri::command]
async fn qr_generate(text: String) -> Result<Vec<Vec<bool>>, String> {
    background(move || sun_core::qr::encode(&text)).await
}

/// Copy the generated QR as an opaque RGBA image to the Windows clipboard.
#[tauri::command]
async fn qr_copy(text: String) -> Result<(), String> {
    background(move || {
        let modules = sun_core::qr::encode(&text)?;
        let (side, pixels) = sun_core::qr::rgba(&modules);
        sun_platform::clipboard_set_image(side, side, pixels)
    })
    .await
}

#[derive(serde::Serialize)]
struct OmnigetInstallation {
    available: bool,
    path: Option<String>,
    missing_tools: Vec<String>,
}

fn omniget_installation(configured: &str) -> OmnigetInstallation {
    let path = sun_platform::omniget::find_cli(configured);
    OmnigetInstallation {
        available: path.is_some(),
        path: path.map(|p| p.to_string_lossy().into_owned()),
        missing_tools: sun_platform::omniget::missing_download_tools(),
    }
}

#[tauri::command]
async fn video_check(app: tauri::AppHandle) -> Result<OmnigetInstallation, String> {
    let configured = app
        .state::<SettingsState>()
        .0
        .lock()
        .unwrap()
        .omniget_cli_path
        .clone();
    background(move || Ok(omniget_installation(&configured))).await
}

#[derive(Clone, serde::Serialize)]
struct OmnigetSetupProgress {
    message: &'static str,
    percent: Option<u8>,
}

fn emit_omniget_setup(app: &tauri::AppHandle, message: &'static str, percent: Option<u8>) {
    let _ = app.emit(
        "sunny:omniget-setup",
        OmnigetSetupProgress { message, percent },
    );
}

/// `reqwest` is built without a bundled TLS provider, and the updater only
/// installs one when its own network call runs first. Without this, HTTPS from
/// any other command panics before connecting.
fn ensure_tls_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

fn download_failure(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "OmniGet download timed out before it finished. Nothing was installed; check your connection and try again.".into()
    } else if error.is_connect() {
        "Could not connect to the official OmniGet download server. Check your connection and try again.".into()
    } else {
        "The OmniGet download failed before it finished. Nothing was installed; try again.".into()
    }
}

/// Blocking on purpose: a hard deadline and an unwind boundary keep this thread
/// from ever leaving a caller without an answer.
fn fetch_package(progress: &dyn Fn(Option<u8>)) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use sun_core::omniget_setup::{MAX_PACKAGE_BYTES, PACKAGE_URL};
    ensure_tls_provider();
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3
                || !sun_core::omniget_setup::allowed_package_url(attempt.url())
            {
                attempt.error("OmniGet package redirected outside the trusted download hosts.")
            } else {
                attempt.follow()
            }
        }))
        .user_agent(concat!("Sunny/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Could not initialize the secure downloader.")?;
    let mut response = client
        .get(PACKAGE_URL)
        .send()
        .map_err(|error| download_failure(&error))?;
    if !response.status().is_success() {
        return Err(format!(
            "The official OmniGet CLI package is currently unavailable (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let total = response.content_length();
    if total
        .map(|size| size > MAX_PACKAGE_BYTES as u64)
        .unwrap_or(false)
    {
        return Err(
            "The OmniGet package exceeds the safe size limit. Nothing was installed.".into(),
        );
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut last_percent = Some(0u8);
    let mut reported_at = 0;
    loop {
        let read = response.read(&mut buffer).map_err(|error| {
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) {
                "OmniGet download timed out before it finished. Nothing was installed; check your connection and try again."
            } else {
                "OmniGet download stalled or was interrupted. Nothing was installed; try again."
            }
        })?;
        if read == 0 {
            break;
        }
        if read > MAX_PACKAGE_BYTES.saturating_sub(bytes.len()) {
            return Err(
                "The OmniGet package exceeds the safe size limit. Nothing was installed.".into(),
            );
        }
        bytes.extend_from_slice(&buffer[..read]);
        let percent = total
            .filter(|size| *size > 0)
            .map(|size| ((bytes.len() as u64 * 100 / size).min(100)) as u8);
        if percent != last_percent || bytes.len() - reported_at >= 1024 * 1024 {
            progress(percent);
            last_percent = percent;
            reported_at = bytes.len();
        }
    }
    if bytes.is_empty() {
        return Err(
            "The official OmniGet download returned no data. Nothing was installed.".into(),
        );
    }
    Ok(bytes)
}

fn install_cli_package(
    app: &tauri::AppHandle,
    target: &std::path::Path,
) -> Result<OmnigetInstallation, String> {
    let bytes = fetch_package(&|percent| {
        emit_omniget_setup(app, "Downloading official OmniGet CLI…", percent)
    })?;
    emit_omniget_setup(app, "Verifying SHA-256 and extracting CLI…", None);
    sun_core::omniget_setup::install_cli_package(&bytes, target)?;
    Ok(omniget_installation(&target.to_string_lossy()))
}

/// Explicit Settings action: fetch only the pinned official CLI, verify its hash,
/// and atomically install it. No installer, shell, or downloaded program is run.
#[tauri::command]
async fn video_setup(app: tauri::AppHandle) -> Result<OmnigetInstallation, String> {
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err("OmniGet setup is available for Windows x64 only.".into());
    }
    let state = app.state::<OmnigetSetupState>();
    let guard = state.reserve(&app.state::<sun_core::video::Downloads>())?;
    let target = sun_platform::omniget::managed_cli_path()
        .filter(|path| path.is_absolute() && !path.to_string_lossy().starts_with(r"\\"))
        .ok_or("The local Sunny tools folder is unavailable.")?;
    emit_omniget_setup(&app, "Downloading official OmniGet CLI (~25 MB)…", Some(0));
    let handle = app.clone();
    let result = background(move || install_cli_package(&handle, &target)).await;
    drop(guard);
    let result = result?;
    emit_omniget_setup(
        &app,
        "OmniGet CLI installed. Additional engines are prepared on first use.",
        Some(100),
    );
    Ok(result)
}

#[tauri::command]
fn video_status(
    downloads: tauri::State<'_, sun_core::video::Downloads>,
) -> Result<sun_core::video::Status, String> {
    downloads.status()
}

#[tauri::command]
fn video_cancel(
    id: u64,
    downloads: tauri::State<'_, sun_core::video::Downloads>,
) -> Result<(), String> {
    downloads.cancel(id)
}

fn emit_video_status(app: &tauri::AppHandle, status: sun_core::video::Status) {
    let _ = app.emit_to("main", "sunny:video-progress", status);
}

/// Streams one CLI run into the shared job state and always finishes it.
fn run_download(
    app: tauri::AppHandle,
    path: std::path::PathBuf,
    args: Vec<String>,
    id: u64,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) {
    use sun_core::video::Downloads;
    let result = sun_platform::omniget::download(&path, &args, cancel, |line| {
        if let Some(update) = sun_core::video::parse_line(line) {
            if let Some(status) = app.state::<Downloads>().update(id, update) {
                emit_video_status(&app, status);
            }
        }
    });
    if let Some(status) = app.state::<Downloads>().finish(id, result) {
        emit_video_status(&app, status);
    }
}

/// Download explicitly through the separately installed OmniGet CLI, never a shell.
#[tauri::command]
async fn video_start(
    app: tauri::AppHandle,
    url: String,
    quality: String,
) -> Result<sun_core::video::Status, String> {
    use sun_core::video::Downloads;
    let configured = app
        .state::<SettingsState>()
        .0
        .lock()
        .unwrap()
        .omniget_cli_path
        .clone();
    background(move || {
        let output = app
            .path()
            .download_dir()
            .map_err(|_| "Windows Downloads folder is unavailable.")?;
        let args = sun_core::video::download_args(&url, &quality, &output.to_string_lossy())?;
        let path = sun_platform::omniget::find_cli(&configured).ok_or(
            "OmniGet CLI was not found. Install OmniGet and select omniget-cli.exe in Settings → General.",
        )?;
        let (status, cancel) = app
            .state::<OmnigetSetupState>()
            .begin_download(&app.state::<Downloads>())?;
        let worker = app.clone();
        let spawned = std::thread::Builder::new()
            .name("sunny-video-download".into())
            .spawn({
                let id = status.id;
                move || run_download(worker, path, args, id, cancel)
            });
        if spawned.is_err() {
            let message = "Could not start the download worker.".to_string();
            if let Some(failed) = app.state::<Downloads>().finish(status.id, Err(message.clone())) {
                emit_video_status(&app, failed);
            }
            return Err(message);
        }
        Ok(status)
    })
    .await
}

#[tauri::command]
async fn video_reveal(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let status = app.state::<sun_core::video::Downloads>().status()?;
    if status.id != id || status.phase != sun_core::video::Phase::Complete {
        return Err("There is no completed download to reveal.".into());
    }
    let path = status
        .file_path
        .ok_or("The saved file path is unavailable.")?;
    background(move || {
        if !sun_platform::omniget::is_local_file(std::path::Path::new(&path)) {
            return Err("The downloaded file could not be found on this computer.".into());
        }
        reveal_path(path)
    })
    .await
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

/// Remove one clipboard entry and persist the change.
#[tauri::command]
fn clip_delete(
    id: u64,
    app: tauri::AppHandle,
    store: tauri::State<'_, ClipboardStore>,
) -> Result<bool, String> {
    let deleted = store.delete(id);
    if deleted {
        if let Err(error) = store.save() {
            let _ = app.emit_to(
                "main",
                "sunny:launcher-warning",
                format!("Entry removed, but the clipboard history could not be saved: {error}"),
            );
        }
    }
    Ok(deleted)
}

/// Copy text, hide the launcher so focus returns to the previous window, then
/// press Ctrl+V. Best effort: if SendInput fails, the text stays copied.
#[tauri::command]
async fn paste_to_active_window(app: tauri::AppHandle, text: String) -> Result<(), String> {
    {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
        clipboard
            .set_text(&text)
            .map_err(|e| format!("Could not copy to the clipboard: {e}"))?;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    background(move || {
        // Give the OS time to move focus back before sending Ctrl+V.
        std::thread::sleep(std::time::Duration::from_millis(220));
        let _ = sun_platform::send_paste();
        Ok(())
    })
    .await
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
async fn search_files(
    query: String,
    app: tauri::AppHandle,
    limit: Option<usize>,
) -> Result<sun_core::files::FileSearchResult, String> {
    background(move || {
        let state = app.state::<FileIndexState>();
        let ready = state.ready.load(std::sync::atomic::Ordering::Relaxed);
        let files = Arc::clone(&*state.files.lock().map_err(|_| "File index unavailable")?);
        let total = files.len();
        let items = sun_core::files::search(&files, &query, limit.unwrap_or(60).min(60));
        Ok(sun_core::files::FileSearchResult {
            items,
            ready,
            total,
        })
    })
    .await
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
            match st.files.lock() {
                Ok(mut slot) => {
                    *slot = Arc::new(files);
                    st.ready.store(true, Ordering::Relaxed);
                }
                Err(_) => {
                    let _ = handle.emit_to(
                        "main",
                        "sunny:launcher-warning",
                        "The file index could not be updated. Rebuild it in Settings.",
                    );
                }
            }
        }
        let _ = handle.emit("sunny:files-updated", ());
    });
}

/// Buka file dengan aplikasi default (ShellExecuteW; aman untuk semua nama file).
#[tauri::command]
async fn open_path(path: String) -> Result<(), String> {
    background(move || sun_platform::shell_open(&path)).await
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
        .manage(AppState(index::SnapshotIndex::default()))
        .manage(ClipboardStore::new())
        .manage(sun_core::video::Downloads::default())
        .manage(OmnigetSetupState::default())
        .manage(SettingsState(Mutex::new(Settings::load())))
        .manage(SettingsTransactions(Mutex::new(())))
        .manage(LauncherState(Mutex::new(
            sun_core::launcher::LauncherHistory::load(),
        )))
        .manage(WinSpaceState(Mutex::new(None)))
        .manage(WindowShows::default())
        .manage(BackdropState(Mutex::new(None)))
        .manage(SettingsBackdropState(Mutex::new(None)))
        .manage(FileIndexState {
            files: Mutex::new(Arc::new(Vec::new())),
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
            qr_generate,
            qr_copy,
            video_check,
            video_setup,
            video_status,
            video_start,
            video_cancel,
            video_reveal,
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

            // HTTPS commands need a process-wide TLS provider (see ensure_tls_provider).
            ensure_tls_provider();

            // ── Setelan aktif (sudah dimuat sebelum window dibuat) ──
            let active = app.state::<SettingsState>().0.lock().unwrap().clone();

            // Saved intent alone does not prove the Run entry exists. Repair a missing
            // entry or old executable path after install/update on every application start.
            if let Err(error) = sun_platform::set_autostart(active.open_at_login) {
                eprintln!("Sunny startup registration: {error}");
            }

            // Saved hotkey first, then the existing Alt+Space fallback. Never
            // abort startup: the tray can still open Sunny and Settings.
            if register_hotkey(app.handle(), &active.hotkey).is_err() {
                match register_hotkey(app.handle(), "Alt+Space") {
                    Ok(()) => {
                        // Keep the in-memory value aligned with the working binding.
                        app.state::<SettingsState>().0.lock().unwrap().hotkey = "Alt+Space".into();
                    }
                    Err(error) => eprintln!(
                        "Sunny could not register a global hotkey ({error}); open it from the tray."
                    ),
                }
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
                if let Err(error) = state_handle.state::<AppState>().0.refresh(load_app_index) {
                    let _ = handle.emit_to("main", "sunny:launcher-warning", error);
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
                                    if let Err(error) = store.save() {
                                        let _ = handle.emit_to(
                                            "main",
                                            "sunny:launcher-warning",
                                            format!(
                                                "Clipboard history could not be saved: {error}"
                                            ),
                                        );
                                    }
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
                        match st.files.lock() {
                            Ok(mut slot) => {
                                *slot = Arc::new(files);
                                st.ready.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            Err(_) => {
                                let _ = handle.emit_to(
                                    "main",
                                    "sunny:launcher-warning",
                                    "The file index could not be built.",
                                );
                            }
                        }
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
    fn tls_provider_is_installed_before_https_commands_run() {
        ensure_tls_provider();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }

    #[test]
    fn panicking_background_task_answers_with_an_error_instead_of_hanging() {
        let result = tauri::async_runtime::block_on(background::<()>(|| panic!("fixture panic")));
        assert!(result.unwrap_err().contains("unexpectedly"));
    }

    #[test]
    #[ignore = "network: downloads the pinned official OmniGet package"]
    fn live_package_downloads_verifies_and_installs() {
        let bytes = fetch_package(&|_| {}).expect("official OmniGet package downloads");
        let dir = std::env::temp_dir().join(format!("sunny-live-setup-{}", std::process::id()));
        let target = dir.join("omniget-cli.exe");
        sun_core::omniget_setup::install_cli_package(&bytes, &target).expect("verified install");
        assert!(target.is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn setup_and_video_download_are_mutually_exclusive_and_release_their_reservations() {
        let setup = OmnigetSetupState::default();
        let jobs = sun_core::video::Downloads::default();
        let guard = setup.reserve(&jobs).unwrap();
        assert!(setup.reserve(&jobs).is_err());
        assert!(setup.begin_download(&jobs).is_err());
        drop(guard);
        let (job, _) = setup.begin_download(&jobs).unwrap();
        assert!(setup.reserve(&jobs).is_err());
        jobs.finish(job.id, Err("fixture failure".into()));
        assert!(setup.reserve(&jobs).is_ok());
    }

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
