//! Sunny core (port GPL-3.0 dari vicinae).
//!
//! Ace — ini port dari `src/lib` vicinae ke Rust:
//! - `fuzzy`  → [crate::search] (mesin pencarian fuzzy + ranking)
//! - `services/app*` → [crate::apps] (indeks aplikasi)

pub mod apps;
pub mod calc;
pub mod clipboard;
pub mod emoji;
pub mod files;
pub(crate) mod fsutil;
pub mod launcher;
pub mod lorem;
pub mod match_rank;
pub mod omniget_setup;
pub mod qr;
pub mod search;
pub mod settings;
pub mod video;
pub mod websearch;
