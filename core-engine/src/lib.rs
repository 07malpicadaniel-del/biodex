//! Core deterministic engine for BioDex (Phase 1 MVP).
//!
//! Provides procedural species card generation, declarative TOML mod parsing,
//! and ECDSA P-256 capture payload verification without network or database I/O.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod crypto;
pub mod mods;
pub mod taxonomy;
