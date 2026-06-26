//! Core data models and merge logic for the Xavier native tool.
//!
//! This crate is deliberately the *mechanical* half of Xavier's multi-model
//! review pipeline. It takes findings that have **already been parsed** out of
//! each model's output and buckets them by exact, normalized `file:line`
//! reference. It does NOT do any semantic or paraphrase adjudication — that is
//! a later model pass. The contract that guarantees this separation is the
//! [`MergeResult::unmatched`] array: any finding the mechanical matcher cannot
//! place (because it lacks a usable location) is surfaced there rather than
//! guessed at.
//!
//! Phase 1 implements a **trivial exact-match merge** that mirrors the bucket
//! semantics of `xavier/deps/multi-model-dispatch/parse.sh`. Fuzzy / near-
//! duplicate matching is explicitly deferred to a later phase.

pub mod merge;
pub mod model;
pub mod render;

pub use merge::merge;
pub use model::{CanonRef, Finding, MatchedPair, MergeInput, MergeResult};
pub use render::debate_markdown;
