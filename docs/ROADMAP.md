# Roadmap Sunny

Port penuh Vicinae (C++23/Qt) → Rust + Tauri v2. Windows-first, GPL-3.0.

## Fase 0 — Fondasi (saat ini)
- [x] Keputusan lisensi: port penuh → GPL-3.0
- [x] Keputusan stack UI: Tauri v2 (WebView2) demi tampilan Raycast-look
- [x] Keputusan platform: Windows dulu
- [x] Repo, LICENSE, NOTICE, README, ROADMAP, PORT-MAP
- [ ] Scaffold workspace Cargo (`crates/sun-core`, `crates/sun-platform`)
- [ ] Scaffold aplikasi Tauri (`apps/sunny`)
- [ ] Clone shallow repo vicinae sebagai referensi porting (jangan dimodifikasi)
- [ ] Window overlay ala Raycast: frameless, selalu di atas, muncul tepat di tengah sesuai monitor aktif, hotkey global (default: Alt+Space)

## Fase 1 — MVP (launcher usable)
- [ ] Mesin pencarian fuzzy (port konsep dari vicinae) + ranking berbasis frekuensi & recency
- [ ] Indeks aplikasi Windows (Start Menu + installed apps)
- [ ] Kalkulator
- [ ] UI list view ala Raycast (tampilan + binding keyboard ↑↓, Enter, Esc, Tab)
- [ ] Peluncuran aplikasi (rilis proses dengan benar)
- [ ] Autorun saat start (opsional)

## Fase 2 — Fitur inti ala vicinae
- [ ] Riwayat clipboard (daemon + penyimpanan terenkripsi/opsional)
- [ ] Text expander / snippet
- [ ] Emoji picker
- [ ] File search (indeks cepat ala Everything/ripgrep)
- [ ] Window switcher

## Fase 3 — Sistem ekstensi
- [ ] Desain API extension Sunny sendiri
- [ ] Kompatibilitas subset perintah skrip Raycast
- [ ] (Opsional/bertahap) Kompatibilitas ekstensi React Raycast

## Fase 4 — Paritas dengan vicinae
- [ ] Store ekstensi
- [ ] AI chat / dictation (stub integrasi)
- [ ] Kontrol volume, font browser, dsb.
- [ ] Linux (wlr/X11) dan macOS

## Prinsip porting
1. **UI ditulis ulang, bukan diterjemahkan**: QML → web UI mirip Raycast; logika C++ → Rust idiomatis (aman, tanpa tandingan 1:1 baris per baris).
2. **Referensi dari vicinae dipetakan** di `docs/PORT-MAP.md` — setiap modul dicantumkan file sumber upstream-nya.
3. **Windows dulu**, abstraksi di `sun-platform` agar Linux/macOS menyusul tanpa refactor besar.
4. Setiap fase harus membuahkan aplikasi yang *usable*, bukan setengah jalan.
