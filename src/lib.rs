//! Moerae: a queryable hierarchical AI memory system for AI agents.
//!
//! Two data verbs: `put` (store content) and `search` (retrieve content).
//! Lifecycle handles forgetting — no delete verb needed.
//!
//! # Quick Start
//!
//! ```no_run
//! use moerae::Moerae;
//!
//! let m = Moerae::init("my-project").unwrap();
//! let mut conv = m.create_conversation().unwrap();
//!
//! conv.put("The capital of France is Paris", None, false).unwrap();
//!
//! let results = conv.search("capital of France", None, None).unwrap();
//! for item in &results.items {
//!     println!("{}", item.data);
//! }
//! ```

mod conversation;
mod project;
mod segment;
mod storage;

pub mod config;
pub mod embedding;
pub mod error;
pub mod index;
pub mod types;

pub use config::Config;
pub use conversation::Conversation;
pub use error::*;
pub use project::{Moerae, MoeraeBuilder};
pub use types::*;
