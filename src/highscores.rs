//! Local high-score table — deviation D3's replacement for the original's
//! dead PHP endpoints.
//!
//! `highscores.txt` lives under the user's data directory by default, or at
//! `CURVEBALL_HIGHSCORES` when that override is set: 10 lines of
//! `name<TAB>level<TAB>score`. A missing or corrupt file (or line) falls back to
//! the defaults (name "none", level 0, score 0). All I/O failures degrade
//! gracefully — the game never crashes over its score file.

use std::path::PathBuf;

use crate::consts::HIGH_SCORE_ROWS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub level: u32,
    pub score: i64,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            name: "none".to_owned(),
            level: 0,
            score: 0,
        }
    }
}

#[derive(Debug)]
pub struct ScoreTable {
    /// Always exactly [`HIGH_SCORE_ROWS`] entries, sorted descending by score.
    entries: Vec<Entry>,
    path: PathBuf,
}

fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CURVEBALL_HIGHSCORES") {
        return PathBuf::from(path);
    }
    user_data_dir()
        .unwrap_or_else(executable_dir)
        .join("curveball")
        .join("highscores.txt")
}

fn executable_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default()
}

fn user_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local").join("share"))
            })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        None
    }
}

fn parse_line(line: &str) -> Option<Entry> {
    let mut fields = line.split('\t');
    let name = fields.next()?.to_owned();
    let level = fields.next()?.parse().ok()?;
    let score = fields.next()?.parse().ok()?;
    if fields.next().is_some() {
        return None;
    }
    Some(Entry { name, level, score })
}

fn normalize_entries(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.score));
    entries.truncate(HIGH_SCORE_ROWS);
    entries.resize_with(HIGH_SCORE_ROWS, Entry::default);
    entries
}

fn replace_file(tmp_path: &std::path::Path, path: &std::path::Path) -> std::io::Result<()> {
    let backup_path = path.with_extension("txt.bak");
    let _ = std::fs::remove_file(&backup_path);
    if path.exists() {
        std::fs::rename(path, &backup_path)?;
    }

    match std::fs::rename(tmp_path, path) {
        Ok(()) => {
            let _ = std::fs::remove_file(backup_path);
            Ok(())
        },
        Err(err) => {
            if backup_path.exists() {
                let _ = std::fs::rename(&backup_path, path);
            }
            Err(err)
        },
    }
}

impl ScoreTable {
    /// Load from the default high-score path.
    #[must_use]
    pub fn load() -> Self {
        Self::load_from(default_path())
    }

    /// Load from an explicit path (used by tests).
    #[must_use]
    pub fn load_from(path: PathBuf) -> Self {
        let entries: Vec<Entry> = std::fs::read_to_string(&path)
            .map(|text| {
                text.lines()
                    .map(|line| parse_line(line).unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
        let entries = normalize_entries(entries);
        Self { entries, path }
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Qualification mirrors the original `checkscore.php` contract: strictly
    /// greater than the 10th entry's score.
    #[must_use]
    pub fn qualifies(&self, score: i64) -> bool {
        self.entries.last().is_some_and(|last| score > last.score)
    }

    /// Insert sorted descending (after any equal scores), truncating to 10.
    pub fn insert(&mut self, name: String, level: u32, score: i64) {
        let at = self
            .entries
            .iter()
            .position(|e| e.score < score)
            .unwrap_or(self.entries.len());
        self.entries.insert(at, Entry { name, level, score });
        self.entries.truncate(HIGH_SCORE_ROWS);
    }

    /// Persist; on failure log via `eprintln!` and continue (never crash the
    /// game loop over the score file).
    pub fn save(&self) {
        use std::fmt::Write as _;
        let mut text = String::new();
        for e in &self.entries {
            // Writing to a String cannot fail.
            let _ = writeln!(text, "{}\t{}\t{}", e.name, e.level, e.score);
        }
        if let Some(parent) = self.path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            eprintln!("curveball: failed to create {}: {err}", parent.display());
            return;
        }

        let tmp_path = self.path.with_extension("txt.tmp");
        let result =
            std::fs::write(&tmp_path, text).and_then(|()| replace_file(&tmp_path, &self.path));
        if let Err(err) = result {
            let _ = std::fs::remove_file(&tmp_path);
            eprintln!("curveball: failed to write {}: {err}", self.path.display());
        }
    }
}
