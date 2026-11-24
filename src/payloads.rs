//! What crosses the network. Trees and progress travel as their own serde JSON, the same shape
//! the data files and the save file use, so the wire has no second schema to keep in step.
//! Registered (version "1", optional) in `Hooks::register`: on a server without Skills the screen
//! says so and nothing is earned.
use mod_api::{PayloadCodec, ToClient, ToServer};
use std::sync::OnceLock;

/// Server → client, on join and after each data reload: every loaded tree (`tree::Trees`).
#[derive(PayloadCodec, Clone)]
pub struct TreesSync {
    pub json: String,
}

/// Server → client, on join and on every change: this player's `progress::Progress`.
#[derive(PayloadCodec, Clone)]
pub struct ProgressSync {
    pub json: String,
}

/// Client → server: a click on an open node. The server checks it and answers with progress.
#[derive(PayloadCodec, Clone)]
pub struct Unlock {
    pub tree: String,
    pub node: String,
}

pub static TREES: OnceLock<ToClient<TreesSync>> = OnceLock::new();
pub static PROGRESS: OnceLock<ToClient<ProgressSync>> = OnceLock::new();
pub static UNLOCK: OnceLock<ToServer<Unlock>> = OnceLock::new();
