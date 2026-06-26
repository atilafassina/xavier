//! Core data models and merge logic for the Xavier native tool.
//!
//! This crate is the *mechanical* half of Xavier's multi-model review
//! pipeline. It owns three responsibilities, each in its own module:
//!
//! - [`findings`] — parse a model's review Markdown into typed [`Finding`]s,
//!   replacing the brittle `awk` scraping that used to live in the shell.
//! - [`refs`] — canonicalize free-form `file:line` / `file:line-range`
//!   references into the stable [`CanonRef`] matching key.
//! - [`merge`] — bucket findings into `consensus` / `blindspot` / `dispute`,
//!   plus an explicit `unmatched` residue, using exact-location matching and
//!   pure-Rust textual near-duplicate detection ([`similarity`]).
//!
//! The mechanical / semantic boundary is the [`MergeResult::unmatched`] array:
//! findings the matcher cannot confidently place — no usable location, or a
//! same-location counterpart that fell below the similarity threshold — are
//! surfaced there for a downstream model pass rather than guessed at. The
//! buckets `consensus` / `blindspot` / `dispute` are final and pass through the
//! model pass untouched.
//!
//! A fourth, orthogonal module — [`cache`] — memoizes the deterministic
//! subcommands on disk: because the merge is a pure function of its input and
//! the binary version, identical inputs can be served byte-for-byte from a
//! content-addressed cache instead of recomputed. It is a transparent
//! optimization layered above the merge, not part of the determinism boundary.

pub mod cache;
pub mod findings;
pub mod merge;
pub mod model;
pub mod refs;
pub mod render;
pub mod similarity;

pub use cache::{Cache, CACHE_DIR_ENV};
pub use findings::parse_findings;
pub use merge::merge;
pub use model::{CanonRef, Finding, MatchedPair, MergeInput, MergeResult, MergeTextInput};
pub use render::debate_markdown;
pub use similarity::{similarity, SIMILARITY_THRESHOLD};
