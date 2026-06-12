//! Local high-score table — deviation D3's replacement for the original's
//! dead PHP endpoints.
//!
//! `highscores.txt` lives beside the executable: 10 lines of
//! `name<TAB>level<TAB>score`. A missing or corrupt file (or line) falls back
//! to the defaults (name "none", level 0, score 0). All I/O failures degrade
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
    pub entries: Vec<Entry>,
    path: PathBuf,
}

fn default_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default()
        .join("highscores.txt")
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

impl ScoreTable {
    /// Load from the default location beside the executable.
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
        if let Err(err) = std::fs::write(&self.path, text) {
            eprintln!("curveball: failed to write {}: {err}", self.path.display());
        }
    }
}
