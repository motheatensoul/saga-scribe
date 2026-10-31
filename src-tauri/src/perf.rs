//! # Performance log
//!
//! Started with `--perf-log` (or `--perf-log=<file>`; `-perf-log` works too), or the environment
//! variable `SAGA_SCRIBE_PERF_LOG` (`1`, or a file), the app writes how long
//! each stage of its work takes to a file: the backend's own, and the
//! frontend's, which it sends with [`perf_record`]. Off, recording costs a
//! check of a flag.
//!
//! One JSON object per line: `t` (when the stage ended, milliseconds since
//! the Unix epoch, the same clock on both sides), `side` (`rust` or `ui`),
//! `stage`, `ms` (its duration) and the stage's own fields, such as sizes.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct PerfLog {
    path: PathBuf,
    file: Mutex<File>,
}

static LOG: OnceLock<PerfLog> = OnceLock::new();

/// Where the performance log was asked for: `Some(None)` for the default
/// place, `Some(Some(file))` for a file of the user's choice.
fn requested() -> Option<Option<PathBuf>> {
    for arg in std::env::args().skip(1) {
        // `-perf-log` too
        let arg = arg.trim_start_matches('-');
        if arg == "perf-log" {
            return Some(None);
        }
        if let Some(path) = arg.strip_prefix("perf-log=") {
            return Some(Some(PathBuf::from(path)));
        }
    }
    match std::env::var("SAGA_SCRIBE_PERF_LOG") {
        Ok(value) if value.is_empty() || value == "0" => None,
        Ok(value) if value == "1" => Some(None),
        Ok(value) => Some(Some(PathBuf::from(value))),
        Err(_) => None,
    }
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

/// Starts the log if it was asked for, by default as `perf-<time>.jsonl` in
/// `default_dir`. Returns its path.
pub fn init(default_dir: &Path) -> Option<PathBuf> {
    let path = requested()?.unwrap_or_else(|| default_dir.join(format!("perf-{}.jsonl", now_ms() as u64 / 1000)));
    start(path)
}

/// Starts the log at `path`.
fn start(path: PathBuf) -> Option<PathBuf> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match File::create(&path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Performance log {} could not be created: {}", path.display(), e);
            return None;
        }
    };
    let log = LOG.get_or_init(|| PerfLog { path, file: Mutex::new(file) });
    record("rust", "perf.start", 0.0, [("version", Value::from(env!("CARGO_PKG_VERSION")))]);
    Some(log.path.clone())
}

/// Whether the log is being written.
pub fn enabled() -> bool {
    LOG.get().is_some()
}

fn write_line(entry: Map<String, Value>) {
    if let Some(log) = LOG.get() {
        let mut line = Value::Object(entry).to_string();
        line.push('\n');
        if let Ok(mut file) = log.file.lock() {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

/// Logs that `stage` took `ms`, with `fields`.
pub fn record<const N: usize>(side: &str, stage: &str, ms: f64, fields: [(&str, Value); N]) {
    if !enabled() {
        return;
    }
    let mut entry = Map::new();
    entry.insert("t".into(), now_ms().into());
    entry.insert("side".into(), side.into());
    entry.insert("stage".into(), stage.into());
    entry.insert("ms".into(), ((ms * 1000.0).round() / 1000.0).into());
    for (name, value) in fields {
        entry.insert(name.into(), value);
    }
    write_line(entry);
}

/// Times a stage of the backend's work: logged when [`Timer::done`] is called.
pub struct Timer(Option<Instant>);

/// Starts timing a stage (nothing, if the log is off).
pub fn timer() -> Timer {
    Timer(enabled().then(Instant::now))
}

impl Timer {
    /// Logs `stage` as taking the time since the timer started, and restarts it.
    pub fn lap<const N: usize>(&mut self, stage: &str, fields: [(&str, Value); N]) {
        if let Some(start) = self.0 {
            record("rust", stage, start.elapsed().as_secs_f64() * 1000.0, fields);
            self.0 = Some(Instant::now());
        }
    }

    /// Logs `stage` as taking the time since the timer started.
    pub fn done<const N: usize>(mut self, stage: &str, fields: [(&str, Value); N]) {
        self.lap(stage, fields);
    }
}

/// A frontend measurement, as `src/lib/perf.ts` sends it.
#[derive(Deserialize)]
pub struct UiEntry {
    t: f64,
    stage: String,
    ms: f64,
    #[serde(default)]
    fields: Map<String, Value>,
}

/// Where the performance log is being written, if it is.
#[tauri::command]
pub fn perf_log_path() -> Option<String> {
    LOG.get().map(|log| log.path.display().to_string())
}

/// Writes the frontend's measurements to the log.
#[tauri::command]
pub fn perf_record(entries: Vec<UiEntry>) {
    for entry in entries {
        let mut line = Map::new();
        line.insert("t".into(), entry.t.into());
        line.insert("side".into(), "ui".into());
        line.insert("stage".into(), entry.stage.into());
        line.insert("ms".into(), ((entry.ms * 1000.0).round() / 1000.0).into());
        line.extend(entry.fields);
        write_line(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_both_sides_are_written_as_json_lines() {
        let path = std::env::temp_dir().join(format!("saga-scribe-perf-test-{}.jsonl", std::process::id()));
        assert_eq!(start(path.clone()), Some(path.clone()));

        let mut timer = timer();
        timer.lap("compile.compile", [("words", 3.into())]);
        perf_record(vec![UiEntry {
            t: 1.5,
            stage: "ui.stall".into(),
            ms: 80.0,
            fields: Map::from_iter([("note".to_string(), Value::from("x"))]),
        }]);

        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(lines[0]["stage"], "perf.start");
        assert_eq!((lines[1]["side"].as_str(), lines[1]["stage"].as_str()), (Some("rust"), Some("compile.compile")));
        assert_eq!(lines[1]["words"], 3);
        assert!(lines[1]["ms"].as_f64().unwrap() >= 0.0);
        assert_eq!(lines[2], serde_json::json!({ "t": 1.5, "side": "ui", "stage": "ui.stall", "ms": 80.0, "note": "x" }));
    }
}
