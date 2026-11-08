//! Sunny core (port GPL-3.0 dari vicinae).
//!
//! Ace — ini port dari `src/lib` vicinae ke Rust:
//! - `fuzzy`  → [crate::search] (mesin pencarian fuzzy + ranking)
//! - `services/app*` → [crate::apps] (indeks aplikasi)

pub mod apps;
pub mod calc;
pub mod clipboard;
pub mod emoji;
pub mod search;