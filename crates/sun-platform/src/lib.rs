//! Sunny platform abstractions (Windows-first).
//!
//! - `extract_icon_png`: ekstrak ikon asli aplikasi (menyelesaikan shortcut .lnk
//!   ke target-nya, sama seperti Explorer) → PNG RGBA.
//!   Konsepnya padanan penanganan ikon app service vicinae.

mod win_space;
pub use win_space::{is_win_space, WinSpaceObserver};

mod autostart;
pub use autostart::set_autostart;

/// Wait for DWM to finish composing a hide/move before capturing the desktop.
/// Call only from a background capture thread, never from the window event loop.
#[cfg(windows)]
pub fn flush_desktop_compositor() {
    // SAFETY: DwmFlush has no pointer or handle preconditions; failure merely
    // leaves capture best-effort (e.g. during a desktop/session transition).
    let _ = unsafe { windows::Win32::Graphics::Dwm::DwmFlush() };
}

#[cfg(not(windows))]
pub fn flush_desktop_compositor() {}

#[cfg(windows)]
mod windows_imp {
    use std::io::Write;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    /// Resolve target sebuah shortcut `.lnk` (path executable aslinya).
    /// Dipakai agar ikon diambil dari aplikasi target, bukan dari file `.lnk`
    /// yang selalu membawa overlay panah shortcut.
    pub fn resolve_lnk_target(lnk: &str) -> Option<String> {
        use windows::core::Interface;
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED, STGM_READ,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

        let wide: Vec<u16> = lnk.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            // Panggilan dari thread background; abaikan bila COM sudah di-init.
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let should_uninit = hr.is_ok();
            let result = (|| -> Option<String> {
                let link: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
                let persist: IPersistFile = link.cast().ok()?;
                persist.Load(PCWSTR(wide.as_ptr()), STGM_READ).ok()?;

                let mut buf = [0u16; 1024];
                link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                if len == 0 {
                    return None;
                }
                Some(String::from_utf16_lossy(&buf[..len]))
            })();
            if should_uninit {
                CoUninitialize();
            }
            result
        }
    }

    /// Path lengkap ke executable/shortcut .lnk → PNG bytes (RGBA), None bila gagal.
    /// Untuk `.lnk`, ikon diambil dari target agar overlay panah shortcut hilang.
    pub fn extract_icon_png(path: &str) -> Option<Vec<u8>> {
        let is_lnk = path.to_ascii_lowercase().ends_with(".lnk");
        let icon_source = if is_lnk {
            resolve_lnk_target(path).unwrap_or_else(|| path.to_string())
        } else {
            path.to_string()
        };
        let wide: Vec<u16> = icon_source
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let mut sfi = SHFILEINFOW::default();
            let flags = SHGFI_ICON | SHGFI_LARGEICON;
            let ok = SHGetFileInfoW(
                PCWSTR(wide.as_ptr()),
                Default::default(),
                Some(&mut sfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                flags,
            );
            if ok == 0 {
                return None;
            }
            let hicon = sfi.hIcon;
            if hicon.is_invalid() {
                return None;
            }
            let result = hicon_to_png(hicon);
            let _ = DestroyIcon(hicon);
            result
        }
    }

    unsafe fn hicon_to_png(
        hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    ) -> Option<Vec<u8>> {
        let mut ii = ICONINFO::default();
        if GetIconInfo(hicon, &mut ii).is_err() {
            return None;
        }
        let color = ii.hbmColor;
        if color.is_invalid() {
            unsafe {
                let _ = DeleteObject(ii.hbmMask.into());
            }
            return None; // ikon mono — jarang di Windows modern; skip
        }

        let result = (|| unsafe {
            let hdc = GetDC(None);
            let png = bitmap_to_png(hdc, color);
            ReleaseDC(None, hdc);
            png
        })();

        unsafe {
            let _ = DeleteObject(color.into());
            let _ = DeleteObject(ii.hbmMask.into());
        }
        result
    }

    unsafe fn bitmap_to_png(
        hdc: windows::Win32::Graphics::Gdi::HDC,
        hbm: windows::Win32::Graphics::Gdi::HBITMAP,
    ) -> Option<Vec<u8>> {
        // 1) Interogasi dimensi
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        if GetDIBits(hdc, hbm, 0, 0, None, &mut bmi, DIB_RGB_COLORS) == 0 {
            return None;
        }
        let w = bmi.bmiHeader.biWidth.unsigned_abs() as usize;
        let h = bmi.bmiHeader.biHeight.unsigned_abs() as usize;
        if w == 0 || h == 0 {
            return None;
        }

        // 2) Salin sebagai 32bpp top-down
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0; // BI_RGB == 0
        bmi.bmiHeader.biHeight = -(bmi.bmiHeader.biHeight.abs());
        let mut buf = vec![0u8; w * h * 4];
        let n = GetDIBits(
            hdc,
            hbm,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        if n == 0 {
            return None;
        }

        // 3) BGRA → RGBA; deteksi alpha kosong (ikon tanpa alpha channel)
        let mut has_alpha = false;
        for px in buf.chunks_exact_mut(4) {
            if px[3] != 0 {
                has_alpha = true;
                break;
            }
        }
        let mut rgba = Vec::with_capacity(buf.len());
        for px in buf.chunks_exact(4) {
            let a = if has_alpha {
                px[3]
            } else {
                if px[0] | px[1] | px[2] == 0 {
                    0
                } else {
                    255
                }
            };
            rgba.extend_from_slice(&[px[2], px[1], px[0], a]);
        }

        // 4) Enkode PNG
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let Ok(mut writer) = encoder.write_header() else {
                return None;
            };
            if writer.write_image_data(&rgba).is_err() {
                return None;
            }
        }
        let _ = std::io::sink().write_all(&[]); // diam: png sudah di-buffer out
        Some(out)
    }

    /// Ambil snapshot area layar (fisik) → PNG RGBA. Dipakai sebagai backdrop
    /// launcher agar opacity hanya memudarkan background, bukan teks/ikon.
    pub fn capture_screen_png(x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u8>> {
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            ReleaseDC, SelectObject, SRCCOPY,
        };
        if w <= 0 || h <= 0 {
            return None;
        }
        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return None;
            }
            let mem = CreateCompatibleDC(Some(screen));
            if mem.is_invalid() {
                ReleaseDC(None, screen);
                return None;
            }
            let bmp = CreateCompatibleBitmap(screen, w, h);
            if bmp.is_invalid() {
                let _ = DeleteDC(mem);
                ReleaseDC(None, screen);
                return None;
            }
            let old = SelectObject(mem, bmp.into());
            let _ = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
            let png = bitmap_to_png(screen, bmp);
            let _ = SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            png
        }
    }
}

#[cfg(windows)]
pub use windows_imp::{capture_screen_png, extract_icon_png, resolve_lnk_target};

/// Satu aplikasi Microsoft Store (AppX/MSIX) yang bisa diluncurkan.
#[derive(Debug, Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct StoreApp {
    /// Nama tampilan dari manifest (`DisplayName`).
    pub name: String,
    /// AppUserModelID, mis. `5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App`.
    pub app_id: String,
    /// Path absolut ke logo `Square44x44Logo` (belum tentu ada).
    pub icon_path: Option<String>,
}

/// Daftar aplikasi Microsoft Store yang terpasang untuk user saat ini.
///
/// Dibaca dari registry AppModel (tidak butuh PowerShell, jadi cepat dan tidak
/// memunculkan jendela konsol). Hanya paket yang punya `<Application>` dengan
/// `Executable` yang dianggap bisa diluncurkan.
#[cfg(windows)]
pub fn list_store_apps() -> Vec<StoreApp> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const REPO: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    let Ok(root) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(REPO) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for pkg in root.enum_keys().flatten() {
        let Ok(key) = root.open_subkey(&pkg) else {
            continue;
        };
        let Ok(root_folder) = key.get_value::<String, _>("PackageRootFolder") else {
            continue;
        };
        let manifest = std::path::Path::new(&root_folder).join("AppxManifest.xml");
        let Ok(xml) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        // Nama paket penuh: `Name_Version_Arch__PublisherId`. AppUserModelID
        // memakai *package family name* = `Name_PublisherId`, bukan nama paket
        // saja — tanpa hash publisher, activation gagal.
        let mut parts = pkg.split('_');
        let Some(pkg_name) = parts.next() else {
            continue;
        };
        let Some(publisher_id) = pkg.rsplit('_').next().filter(|s| !s.is_empty()) else {
            continue;
        };
        let family_name = format!("{pkg_name}_{publisher_id}");
        let fallback_name = package_display_name(pkg_name);
        for app in parse_manifest_apps(&xml, &root_folder) {
            // `ms-resource:` tidak bisa dibaca tanpa MRT Core; pakai nama paket
            // yang sudah "dibersihkan" supaya hasilnya tetap masuk akal.
            let name = if app.name.starts_with("ms-resource:") {
                fallback_name.clone()
            } else {
                app.name.clone()
            };
            out.push(StoreApp {
                name,
                app_id: format!("{family_name}!{}", app.id),
                icon_path: app.icon_path,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.app_id == b.app_id);
    out
}

/// Nama yang bisa dibaca dari nama paket, mis.
/// `Microsoft.WindowsCalculator` → `Windows Calculator`.
#[cfg(windows)]
fn package_display_name(pkg_name: &str) -> String {
    let tail = pkg_name.rsplit('.').next().unwrap_or(pkg_name);
    let mut out = String::with_capacity(tail.len() + 4);
    let mut prev_lower = false;
    for ch in tail.chars() {
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
        out.push(ch);
    }
    if out.trim().is_empty() {
        pkg_name.to_string()
    } else {
        out
    }
}

#[cfg(windows)]
struct ManifestApp {
    id: String,
    name: String,
    icon_path: Option<String>,
}

/// Parser minimal untuk `AppxManifest.xml`.
///
/// Manifest memakai namespace XML, jadi parser generik akan merepotkan. Yang
/// dibutuhkan hanya `<Application Id=...>` beserta `DisplayName` dan
/// `Square44x44Logo` di dalamnya, sehingga pencarian string sudah cukup dan
/// tidak menambah dependensi.
#[cfg(windows)]
fn parse_manifest_apps(xml: &str, root_folder: &str) -> Vec<ManifestApp> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Application ") {
        let tail = &rest[start..];
        let Some(end) = tail.find("</Application>") else {
            break;
        };
        let block = &tail[..end];
        rest = &tail[end + "</Application>".len()..];

        // Windows marks internal components (File Picker, brokers, etc.) as hidden.
        // They can have executables but are not user-launchable app-list entries.
        if attr(block, "AppListEntry=").is_some_and(|value| value.eq_ignore_ascii_case("none")) {
            continue;
        }

        // Hanya aplikasi yang punya Executable (bukan sekadar background task).
        if !block.contains("Executable=") {
            continue;
        }
        let Some(id) = attr(block, "Id=") else {
            continue;
        };
        // DisplayName bisa berupa referensi resource (`ms-resource:...`); pemanggil
        // yang memutuskan fallback bila tidak bisa ditampilkan apa adanya.
        let name = attr(block, "DisplayName=").unwrap_or_else(|| id.clone());
        let icon_path = attr(block, "Square44x44Logo=").map(|rel| {
            let rel = rel.replace('/', "\\");
            format!("{root_folder}\\{rel}")
        });
        out.push(ManifestApp {
            id,
            name,
            icon_path,
        });
    }
    out
}

#[cfg(all(test, windows))]
mod manifest_tests {
    use super::*;

    #[test]
    fn excludes_hidden_windows_file_picker_but_keeps_whatsapp() {
        let xml = r#"<Applications>
          <Application Id="Microsoft.Windows.FilePicker" Executable="FilePicker.exe">
            <uap:VisualElements DisplayName="ms-resource:AppxManifest_DisplayName"
              AppListEntry="none" Square44x44Logo="Assets/FilePicker.png" />
          </Application>
          <Application Id="App" Executable="WhatsApp.exe">
            <uap:VisualElements DisplayName="WhatsApp" Square44x44Logo="Assets/AppList.png" />
          </Application>
        </Applications>"#;
        let apps = parse_manifest_apps(xml, r"C:\Packages");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "WhatsApp");
    }

    #[test]
    fn hidden_application_attribute_is_respected() {
        let xml = r#"<Application Id="Background" Executable="Helper.exe" uap:AppListEntry="none">
          <uap:VisualElements DisplayName="Helper" />
        </Application>"#;
        assert!(parse_manifest_apps(xml, r"C:\Packages").is_empty());
    }

    #[test]
    fn explicit_default_visibility_remains_searchable() {
        let xml = r#"<Application Id="Calculator" Executable="Calculator.exe">
          <uap:VisualElements DisplayName="Calculator" AppListEntry="default" />
        </Application>"#;
        let apps = parse_manifest_apps(xml, r"C:\Packages");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Calculator");
    }
}

/// Ambil nilai atribut XML sederhana, mis. `Id="App"` → `App`.
#[cfg(windows)]
fn attr(block: &str, key: &str) -> Option<String> {
    let start = block.find(key)? + key.len();
    let rest = &block[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &rest[1..];
    let end = body.find(quote)?;
    Some(body[..end].to_string())
}

/// Luncurkan aplikasi Microsoft Store lewat AppUserModelID.
#[cfg(windows)]
pub fn launch_store_app(app_id: &str) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        ApplicationActivationManager, IApplicationActivationManager, AO_NONE,
    };

    let wide: Vec<u16> = app_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let should_uninit = hr.is_ok();
        let result = (|| -> Result<(), String> {
            let manager: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("activation manager: {e}"))?;
            manager
                .ActivateApplication(PCWSTR(wide.as_ptr()), &HSTRING::new(), AO_NONE)
                .map_err(|e| format!("activate: {e}"))?;
            Ok(())
        })();
        if should_uninit {
            CoUninitialize();
        }
        result
    }
}

/// Ikon aplikasi Store: pakai file logo dari paket, fallback ke ekstraksi
/// ikon biasa bila file tidak ada.
///
/// Manifest biasanya menunjuk nama dasar (`Assets\AppList.png`) sementara paket
/// hanya menyimpan varian skala (`AppList.targetsize-256.png`), jadi nama dasar
/// itu perlu diresolusi dulu.
#[cfg(windows)]
pub fn store_app_icon_png(icon_path: Option<&str>) -> Option<Vec<u8>> {
    let path = std::path::Path::new(icon_path?);
    let resolved = resolve_logo_variant(path)?;
    let bytes = std::fs::read(resolved).ok()?;
    if bytes.len() > 64 {
        Some(bytes)
    } else {
        None
    }
}

/// Cari varian logo terbesar yang tersedia untuk nama dasar dari manifest.
#[cfg(windows)]
fn resolve_logo_variant(base: &std::path::Path) -> Option<std::path::PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    let dir = base.parent()?;
    let stem = base.file_stem()?.to_str()?;
    let entries = std::fs::read_dir(dir).ok()?;

    // Pilih skala terbesar; `targetsize-*` = ukuran piksel, `scale-*` = kelipatan.
    let mut best: Option<(u32, std::path::PathBuf)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix(stem) else {
            continue;
        };
        // Hanya varian dari file ini (`AppList.targetsize-256.png`), bukan
        // file lain yang kebetulan berawalan sama.
        let Some(rest) = rest.strip_prefix('.') else {
            continue;
        };
        if !rest.ends_with(".png") || rest.contains("_altform-light") {
            continue;
        }
        let score = rest
            .strip_prefix("targetsize-")
            .and_then(|r| r.split('.').next())
            .and_then(|r| r.parse::<u32>().ok())
            .map(|s| s * 10)
            .or_else(|| {
                rest.strip_prefix("scale-")
                    .and_then(|r| r.split('.').next())
                    .and_then(|r| r.parse::<u32>().ok())
                    .map(|s| s * 100)
            })
            .unwrap_or(0);
        if best.as_ref().map_or(true, |(b, _)| score > *b) {
            best = Some((score, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}

/// Buka file/folder dengan aplikasi default (Windows `ShellExecuteW`).
/// Tidak lewat `cmd /C start`, jadi path dengan `&`, `%`, kutip, dsb. aman.
#[cfg(windows)]
pub fn shell_open(path: &str) -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // Launch may run on a worker thread. Shell verb handlers are often COM objects,
    // so give this thread an apartment; only uninitialize when we initialized it.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let ret = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if initialized {
        unsafe { CoUninitialize() };
    }
    if ret.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("failed to open path (code {})", ret.0 as isize))
    }
}

#[cfg(not(windows))]
pub fn shell_open(path: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Nomor urut global clipboard Windows — naik setiap isi clipboard berubah.
/// Dibandingkan antar-poll untuk mendeteksi perubahan tanpa isi membaca (murah).
#[cfg(windows)]
pub fn clipboard_sequence() -> u32 {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(not(windows))]
pub fn clipboard_sequence() -> u32 {
    0
}

/// Kirim Ctrl+V ke jendela yang sedang aktif (best-effort) —
/// melengkapi "Paste to active window" ala Raycast setelah memilih entri.
#[cfg(windows)]
pub fn send_paste() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };
    let mk = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        mk(VK_CONTROL.0, false),
        mk(VK_V.0, false),
        mk(VK_V.0, true),
        mk(VK_CONTROL.0, true),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    sent == inputs.len() as u32
}

#[cfg(not(windows))]
pub fn send_paste() -> bool {
    false
}

#[cfg(not(windows))]
/// Platform lain belum diimplementasi di fase ini.
pub fn extract_icon_png(_path: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(not(windows))]
pub fn list_store_apps() -> Vec<StoreApp> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn launch_store_app(_app_id: &str) -> Result<(), String> {
    Err("Microsoft Store apps are only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn store_app_icon_png(_icon_path: Option<&str>) -> Option<Vec<u8>> {
    None
}
