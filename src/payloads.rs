//! What crosses the network. Trees and progress travel as their own serde JSON, the same shape
//! the data files and the save file use, so the wire has no second schema to keep in step. A
//! payload string holds at most 4096 bytes, so the JSON travels as a list of parts ([`Json`]).
//! Registered (version "1", optional) in `Hooks::register`: on a server without Skills the screen
//! says so and nothing is earned.
use mod_api::{PayloadCodec, ToClient, ToServer};
use std::sync::OnceLock;

/// A value as JSON, cut into parts that each fit a payload string.
#[derive(PayloadCodec, Clone, Debug, PartialEq)]
pub struct Json {
    pub parts: Vec<String>,
}

/// Bytes per part: under the payload's 4096-byte string limit.
const PART: usize = 4000;

impl Json {
    pub fn of<T: serde::Serialize>(value: &T) -> Self {
        let text = serde_json::to_string(value).unwrap_or_default();
        let mut parts = Vec::new();
        let mut rest = text.as_str();
        while !rest.is_empty() {
            let mut cut = rest.len().min(PART);
            while !rest.is_char_boundary(cut) {
                cut -= 1;
            }
            parts.push(rest[..cut].to_owned());
            rest = &rest[cut..];
        }
        Self { parts }
    }

    pub fn read<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_str(&self.parts.concat()).map_err(|e| e.to_string())
    }
}

/// Server → client, on join and after each data reload: every loaded tree (`tree::Trees`).
#[derive(PayloadCodec, Clone)]
pub struct TreesSync {
    pub json: Json,
}

/// Server → client, on join and on every change: this player's `progress::Progress`.
#[derive(PayloadCodec, Clone)]
pub struct ProgressSync {
    pub json: Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_and_arabic_json_round_trips_in_parts_under_the_limit() {
        let value: Vec<String> = (0..2000).map(|i| format!("مهارة {i}")).collect();
        let json = Json::of(&value);
        assert!(json.parts.len() > 1);
        assert!(json.parts.iter().all(|p| p.len() <= 4096));
        assert_eq!(json.read::<Vec<String>>().unwrap(), value);
    }
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
