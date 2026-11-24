//! Progress on disk: `players/data/<uuid>.skills.json`, beside the player's own file, written
//! whole to a temporary file and renamed over the old one so a crash never leaves half a file.
use crate::progress::Progress;
use std::io;
use std::path::{Path, PathBuf};

const SUFFIX: &str = "skills.json";

pub fn path(directory: &Path, uuid: &str) -> PathBuf {
    directory.join(format!("{uuid}.{SUFFIX}"))
}

/// The saved progress, empty for a player with none. A file that won't parse is kept aside as
/// `<uuid>.skills.json.broken` (never deleted) and the player starts empty.
pub fn read(directory: &Path, uuid: &str) -> Progress {
    let file = path(directory, uuid);
    let Ok(bytes) = std::fs::read(&file) else { return Progress::default() };
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let _ = std::fs::rename(&file, file.with_extension("json.broken"));
        Progress::default()
    })
}

pub fn write(directory: &Path, uuid: &str, progress: &Progress) -> io::Result<()> {
    let file = path(directory, uuid);
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(progress).map_err(io::Error::other)?)?;
    std::fs::rename(&tmp, &file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_broken_files_are_kept() {
        let dir = std::env::temp_dir().join(format!("skills-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = Progress::default();
        p.trees.entry("skills:mining".into()).or_default().xp = 42.5;
        write(&dir, "u1", &p).unwrap();
        assert_eq!(read(&dir, "u1"), p);
        assert_eq!(read(&dir, "nobody"), Progress::default());
        std::fs::write(path(&dir, "u2"), b"{oops").unwrap();
        assert_eq!(read(&dir, "u2"), Progress::default());
        assert!(dir.join("u2.skills.json.broken").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
