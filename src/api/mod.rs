#![allow(unused)]
//! API Client — multi-provider LLM integration.
//!
//! Supports: Anthropic (native), OpenAI-compatible (15+ providers), Google Gemini.
//! Network features are gated behind the `network` feature flag.

pub mod client;
pub mod google_provider;
pub mod openai_compat;
pub mod providers;
pub mod retry;
pub mod streaming;
