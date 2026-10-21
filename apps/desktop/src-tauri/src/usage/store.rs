//! Persistence for the daily usage record.
//!
//! Deliberately a tiny text file rather than JSON: it holds four numbers and a
//! date, it has no dependency to resolve, and it is trivially inspectable by a
//! user who wants to know what Nexora recorded. It never contains prompts,
//! replies, file contents, paths or identifiers.
//!
//! Writes are atomic: a sibling temporary file is written, flushed, synced and
//! renamed over the record. A crash mid-write therefore leaves either the old
//! record or the new one, never a truncated file that would look like a fresh
//! allowance.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use super::{MAX_STORED_USED, RECORD_VERSION};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub version: u32,
    /// Accounting policy that wrote this file.
    pub policy: String,
    /// UTC day, days since the Unix epoch.
    pub day: i64,
    pub used: u32,
    /// Informational only; never used to enforce anything.
    pub updated_ms: i64,
}

pub enum Loaded {
    Missing,
    Record(Record),
    /// The file exists but could not be read as text.
    Unreadable(String),
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Loaded {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Loaded::Missing,
            Err(error) => return Loaded::Unreadable(error.to_string()),
        };

        match parse(&text) {
            Some(record) => Loaded::Record(record),
            None => Loaded::Unreadable("the record is not in the expected format".to_string()),
        }
    }

    pub fn save(&self, record: &Record) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("the usage folder could not be created ({error})"))?;
        }

        let text = encode(record);
        let temp = self.temp_path();

        let write = || -> std::io::Result<()> {
            let mut file = fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, &self.path)?;

            #[cfg(unix)]
            {
                if let Some(parent) = self.path.parent() {
                    if let Ok(dir) = fs::File::open(parent) {
                        let _ = dir.sync_all();
                    }
                }
            }

            Ok(())
        };

        if let Err(error) = write() {
            let _ = fs::remove_file(&temp);
            return Err(format!("the usage record could not be written ({error})"));
        }

        Ok(())
    }

    fn temp_path(&self) -> PathBuf {
        let name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("daily-usage");

        self.path.with_file_name(format!("{name}.tmp"))
    }
}

pub fn encode(record: &Record) -> String {
    format!(
        "version={}\npolicy={}\nday={}\nused={}\nupdated={}\n",
        record.version, record.policy, record.day, record.used, record.updated_ms
    )
}

/// Parses the fixed five-line format. Missing, duplicated or non-numeric keys
/// make the whole record unreadable: a half-parsed counter is worse than none.
pub fn parse(text: &str) -> Option<Record> {
    let mut version = None;
    let mut policy = None;
    let mut day = None;
    let mut used = None;
    let mut updated = None;

    for line in text.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let (key, value) = line.split_once('=')?;

        match key {
            "version" => version = Some(value.parse::<u32>().ok()?),
            "policy" => {
                if value.is_empty() || value.len() > 80 {
                    return None;
                }
                policy = Some(value.to_string());
            }
            "day" => day = Some(value.parse::<i64>().ok()?),
            "used" => used = Some(value.parse::<u32>().ok()?),
            "updated" => updated = Some(value.parse::<i64>().ok()?),
            _ => return None,
        }
    }

    let record = Record {
        version: version?,
        policy: policy?,
        day: day?,
        used: used?,
        updated_ms: updated?,
    };

    if record.version != RECORD_VERSION
        || record.used > MAX_STORED_USED
        || record.day.unsigned_abs() > 4_000_000
    {
        return None;
    }

    Some(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexora-usage-store-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(day: i64, used: u32) -> Record {
        Record {
            version: RECORD_VERSION,
            policy: "nexora-mini-free-generations-v1".to_string(),
            day,
            used,
            updated_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = scratch("roundtrip");
        let store = Store::new(dir.join("daily-usage.txt"));

        store.save(&record(20_750, 12)).expect("save");

        match store.load() {
            Loaded::Record(loaded) => assert_eq!(loaded, record(20_750, 12)),
            _ => panic!("expected a record"),
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let dir = scratch("missing");
        let store = Store::new(dir.join("daily-usage.txt"));

        assert!(matches!(store.load(), Loaded::Missing));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_second_write_replaces_the_first_without_leaving_a_temp_file() {
        let dir = scratch("replace");
        let path = dir.join("daily-usage.txt");
        let store = Store::new(path.clone());

        store.save(&record(20_750, 3)).expect("first");
        store.save(&record(20_750, 4)).expect("second");

        match store.load() {
            Loaded::Record(loaded) => assert_eq!(loaded.used, 4),
            _ => panic!("expected a record"),
        }

        assert!(!dir.join("daily-usage.txt.tmp").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn garbage_is_unreadable_rather_than_a_default() {
        let dir = scratch("garbage");
        let path = dir.join("daily-usage.txt");
        fs::write(&path, "used=3\n").unwrap();

        assert!(matches!(Store::new(path).load(), Loaded::Unreadable(_)));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn negative_and_oversized_counters_are_rejected() {
        let text = "version=1\npolicy=p\nday=1\nused=-2\nupdated=0\n";
        assert!(parse(text).is_none());

        let text = "version=1\npolicy=p\nday=1\nused=1000001\nupdated=0\n";
        assert!(parse(text).is_none());

        let text = "version=1\npolicy=p\nday=1\nused=2\nextra=1\n";
        assert!(parse(text).is_none());
    }

    #[test]
    fn the_encoded_record_has_no_prompts_or_paths() {
        let text = encode(&record(20_750, 5));

        assert_eq!(
            text,
            "version=1\npolicy=nexora-mini-free-generations-v1\nday=20750\nused=5\nupdated=1700000000000\n"
        );
        assert!(!text.contains('/'));
        assert!(!text.contains('\\'));
    }
}
