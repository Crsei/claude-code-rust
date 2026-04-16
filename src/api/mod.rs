//! API Client — multi-provider LLM integration.
//!
//! Supports: Anthropic (native), OpenAI-compatible (15+ providers), Google Gemini.

pub mod client;
pub mod google_provider;
pub mod openai_compat;
pub mod pricing;
pub mod providers;
pub mod retry;
pub mod stream_provider;
pub mod streaming;
