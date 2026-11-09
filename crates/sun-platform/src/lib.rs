//! Sunny platform abstractions (Windows-first).
//!
//! - `extract_icon_png`: ekstrak ikon asli aplikasi (menyelesaikan shortcut .lnk
//!   ke target-nya, sama seperti Explorer) → PNG RGBA.
//!   Konsepnya padanan penanganan ikon app service vicinae.

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

/// Buka file/folder dengan aplikasi default (Windows `ShellExecuteW`).
/// Tidak lewat `cmd /C start`, jadi path dengan `&`, `%`, kutip, dsb. aman.
#[cfg(windows)]
pub fn shell_open(path: &str) -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
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

/// Daftarkan/lepas Sunny di autostart user (`HKCU\...\Run`), tanpa admin.
#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| format!("registry: {e}"))?;

    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("exe path: {e}"))?;
        key.set_value("Sunny", &format!("\"{}\"", exe.display()))
            .map_err(|e| format!("registry: {e}"))?;
    } else {
        match key.delete_value("Sunny") {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("registry: {e}")),
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_autostart(_enabled: bool) -> Result<(), String> {
    Ok(())
}
