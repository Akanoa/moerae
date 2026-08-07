//! Moerae: a queryable hierarchical AI memory system for AI agents.
//!
//! Two data verbs: `put` (store content) and `search` (retrieve content).
//! Lifecycle handles forgetting automatically — content ages out through disuse.
//! `forget` exists for the case lifecycle cannot solve: content that is no longer
//! true. A wrong fact that keeps being retrieved is the *last* thing eviction
//! removes, so superseded content has to be removed on request.
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
//!
//! // Remove a specific fact once it is outdated.
//! let node_id = results.items[0].node_id;
//! m.forget_nodes(&[node_id]).unwrap();
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
