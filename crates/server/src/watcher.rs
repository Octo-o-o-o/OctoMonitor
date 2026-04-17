use std::path::PathBuf;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};

use crate::platform::home_dir;
use crate::state::AppState;

/// Resolve `var` as a path, falling back to `home/<fallback>`.
fn env_path_or(home: &std::path::Path, vars: &[&str], fallback: &str) -> PathBuf {
    for var in vars {
        if let Ok(value) = std::env::var(var) {
            return PathBuf::from(value);
        }
    }
    home.join(fallback)
}

/// Directories to watch for each adapter, relative to home.
fn watch_dirs() -> Vec<PathBuf> {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));

    let claude_base = env_path_or(&home, &["CLAUDE_CONFIG_DIR"], ".claude");
    let codex_base = env_path_or(&home, &["CODEX_HOME"], ".codex");
    let openclaw_base =
        env_path_or(&home, &["OPENCLAW_STATE_DIR", "OPENCLAW_HOME"], ".openclaw");
    let hermes_base = env_path_or(&home, &["HERMES_HOME"], ".hermes");

    let mut dirs = vec![
        claude_base.join("projects"),
        codex_base.join("sessions"),
        openclaw_base.join("agents"),
        // Default Hermes sessions
        hermes_base.join("sessions"),
    ];

    let hermes_profiles = hermes_base.join("profiles");
    if let Ok(entries) = std::fs::read_dir(&hermes_profiles) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                dirs.push(entry.path().join("sessions"));
            }
        }
    }

    dirs
}

/// Spawn a background task that watches adapter session directories and wakes
/// the probe on any file change. Uses debouncing (2 s) to avoid flooding.
pub fn spawn_fs_watcher(state: AppState) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = match new_debouncer(Duration::from_secs(2), tx) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Could not start filesystem watcher: {e}");
                return;
            }
        };

        let dirs = watch_dirs();
        let mut watching = 0usize;
        for dir in &dirs {
            if !dir.exists() {
                tracing::debug!("Watch dir does not exist (yet): {}", dir.display());
                continue;
            }
            match debouncer
                .watcher()
                .watch(dir, notify::RecursiveMode::Recursive)
            {
                Ok(()) => {
                    watching += 1;
                    tracing::info!("Watching {}", dir.display());
                }
                Err(e) => {
                    tracing::warn!("Cannot watch {}: {e}", dir.display());
                }
            }
        }

        if watching == 0 {
            tracing::info!("No adapter directories found to watch; fs watcher idle");
        }

        // Block this thread, forwarding debounced events → probe wake.
        loop {
            match rx.recv() {
                Ok(Ok(events)) => {
                    let dominated_by_data =
                        events.iter().any(|e| e.kind == DebouncedEventKind::Any);
                    if dominated_by_data {
                        tracing::debug!(
                            "FS change detected ({} events), waking probe",
                            events.len()
                        );
                        state.wake_probe_with_reason("fs_watcher");
                    }
                }
                Ok(Err(err)) => {
                    tracing::warn!("Watch error: {err}");
                }
                Err(_) => {
                    // Channel closed — debouncer dropped.
                    tracing::info!("FS watcher channel closed, exiting");
                    break;
                }
            }
        }
    });
}
