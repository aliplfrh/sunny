//! Reproducible, synthetic search benchmark; never reads personal files/history.
use std::time::Instant;
use sun_core::{apps::AppEntry, files::FileEntry};

fn main() {
    let index: Vec<_> = (0..60_000)
        .map(|i| FileEntry {
            name: format!("project-notes-{i}.txt"),
            path: format!(r"C:\Users\Benchmark\Documents\project-{i}\project-notes-{i}.txt"),
            is_dir: false,
        })
        .collect();
    for query in ["no", "notes", "sunny", "Siapa itu jokowi?"] {
        let mut times = Vec::new();
        for _ in 0..15 {
            let start = Instant::now();
            std::hint::black_box(sun_core::files::search(&index, query, 60));
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "files {query:?}: median {:.2} ms, max {:.2} ms",
            times[7], times[14]
        );
    }
    // Model the previous hot path's deep copy of an icon-heavy app index.
    let apps: Vec<_> = (0..400)
        .map(|i| AppEntry {
            name: format!("App {i}"),
            path: format!(r"C:\Benchmark\App-{i}.lnk"),
            icon: Some("x".repeat(32_000)),
            source: "start-menu",
            category: "Application",
        })
        .collect();
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(sun_core::search::rank("App 2", &apps.clone()));
    }
    println!(
        "apps with deep index clone: {:.2} ms/query",
        start.elapsed().as_secs_f64() * 10.0
    );
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(sun_core::search::rank("App 2", &apps));
    }
    println!(
        "apps borrowing snapshot: {:.2} ms/query",
        start.elapsed().as_secs_f64() * 10.0
    );
}
