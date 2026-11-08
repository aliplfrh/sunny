//! Kalkulator. Port konsep backend Soulver di vicinae (`src/lib/soulver`),
//! implementasi Rust: evaluasi ekspresi aritmetika via `meval`.
//!
//! Layak muncul di urutan pertama launcher bila kueri bisa dievaluasi.

/// Evaluasi ekspresi → string hasil yang siap tampil + siap disalin.
/// None bila kueri bukan ekspresi matematis yang valid.
pub fn evaluate(query: &str) -> Option<String> {
    let q = query.trim();
    if q.len() < 2 {
        return None;
    }

    let value = meval::eval_str(q).ok()?;

    // Tolak hasil tak-selesai (pembagian dengan nol dsb.) dan NaN
    if !value.is_finite() {
        return None;
    }

    Some(format_value(value))
}

/// Format angka ala kalkulator: bilangan bulat tanpa titik desimal,
/// selain itu maksimum 10 digit signifikan dengan nol trailing dipangkas.
fn format_value(v: f64) -> String {
    // Angka "praktis bulat" → tampilkan sebagai bulat (mis. 2+2 = 4, bukan 4.0000000001)
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 {
        return format!("{}", v.round() as i64);
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}
