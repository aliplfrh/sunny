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
    /// Path lengkap ke executable/shortcut .lnk → PNG bytes (RGBA), None bila gagal.
    /// SHGetFileInfoW pada .lnk otomatis me-resolve ikon targetnya (perilaku Explorer).
    pub fn extract_icon_png(path: &str) -> Option<Vec<u8>> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
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
            }            let hicon = sfi.hIcon;
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

    unsafe fn bitmap_to_png(hdc: windows::Win32::Graphics::Gdi::HDC, hbm: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<Vec<u8>> {
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
            let a = if has_alpha { px[3] } else { if px[0]|px[1]|px[2] == 0 { 0 } else { 255 } };
            rgba.extend_from_slice(&[px[2], px[1], px[0], a]);
        }

        // 4) Enkode PNG
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let Ok(mut writer) = encoder.write_header() else { return None };
            if writer.write_image_data(&rgba).is_err() {
                return None;
            }
        }
        let _ = std::io::sink().write_all(&[]); // diam: png sudah di-buffer out
        Some(out)
    }
}

#[cfg(windows)]
pub use windows_imp::extract_icon_png;

#[cfg(not(windows))]
/// Platform lain belum diimplementasi di fase ini.
pub fn extract_icon_png(_path: &str) -> Option<Vec<u8>> {
    None
}
