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
///
/// Resolves the home root via `platform::home_dir()` and delegates to the
/// pure `watch_dirs_for_home`, which is unit-testable without touching the
/// process-global `HOME` env.
fn watch_dirs() -> Vec<PathBuf> {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    watch_dirs_for_home(&home)
}

/// Pure helper: given a home directory, return every adapter session
/// directory we should subscribe to. Per-adapter env overrides
/// (CLAUDE_CONFIG_DIR / CODEX_HOME / OPENCLAW_STATE_DIR / OPENCLAW_HOME /
/// HERMES_HOME and P0 adapter homes) still apply because they are checked through `env_path_or`,
/// but the supplied `home` becomes the fallback. Filesystem reads under the
/// Hermes `profiles/` tree are also relative to whatever `home` is given.
pub(crate) fn watch_dirs_for_home(home: &std::path::Path) -> Vec<PathBuf> {
    let claude_base = env_path_or(home, &["CLAUDE_CONFIG_DIR"], ".claude");
    let codex_base = env_path_or(home, &["CODEX_HOME"], ".codex");
    let openclaw_base = env_path_or(home, &["OPENCLAW_STATE_DIR", "OPENCLAW_HOME"], ".openclaw");
    let hermes_base = env_path_or(home, &["HERMES_HOME"], ".hermes");
    let codebuddy_base = env_path_or(home, &["CODEBUDDY_CONFIG_DIR"], ".codebuddy");
    let gemini_base = env_path_or(home, &["GEMINI_HOME"], ".gemini");
    let pi_base = env_path_or(home, &["PI_CODING_AGENT_HOME"], ".pi/agent");
    let opencode_base = env_path_or(home, &["OPENCODE_CONFIG_DIR"], ".local/share/opencode");
    let copilot_base = env_path_or(home, &["COPILOT_HOME"], ".copilot");
    let openhands_base = env_path_or(home, &["OPENHANDS_HOME"], ".openhands");
    let continue_base = env_path_or(home, &["CONTINUE_GLOBAL_DIR"], ".continue");
    let qwen_base = env_path_or(home, &["QWEN_CONFIG_DIR"], ".qwen");
    let kimi_base = env_path_or(home, &["KIMI_CODE_HOME"], ".kimi-code");
    let goose_base = env_path_or(home, &["GOOSE_DATA_DIR"], ".local/share/goose");
    let cursor_base = env_path_or(home, &["CURSOR_AGENT_HOME"], ".cursor");
    let cline_base = env_path_or(home, &["CLINE_HOME", "CLINE_DATA_DIR"], ".cline");
    let kiro_base = env_path_or(home, &["KIRO_HOME"], ".kiro");

    let mut dirs = vec![
        claude_base.join("projects"),
        codex_base.join("sessions"),
        openclaw_base.join("agents"),
        // Default Hermes sessions
        hermes_base.join("sessions"),
        codebuddy_base.join("projects"),
        codebuddy_base.join("sessions"),
        gemini_base.join("tmp"),
        pi_base.join("sessions"),
        opencode_base,
        copilot_base.join("session-state"),
        openhands_base.join("conversations"),
        continue_base.join("sessions"),
        qwen_base.join("projects"),
        kimi_base.join("sessions"),
        goose_base.join("sessions"),
        cursor_base.join("chats"),
        cline_base,
        kiro_base,
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::watch_dirs_for_home;

    #[test]
    fn watch_dirs_for_home_uses_dot_directory_defaults() {
        // With no per-adapter env overrides set, fallback paths must hang off
        // the supplied home and target each adapter's known session dir.
        let temp = tempdir().unwrap();
        let home = temp.path();
        let dirs = watch_dirs_for_home(home);

        let expected = [
            home.join(".claude/projects"),
            home.join(".codex/sessions"),
            home.join(".openclaw/agents"),
            home.join(".hermes/sessions"),
            home.join(".codebuddy/projects"),
            home.join(".codebuddy/sessions"),
            home.join(".gemini/tmp"),
            home.join(".pi/agent/sessions"),
            home.join(".local/share/opencode"),
            home.join(".copilot/session-state"),
            home.join(".openhands/conversations"),
            home.join(".continue/sessions"),
            home.join(".qwen/projects"),
            home.join(".kimi-code/sessions"),
            home.join(".local/share/goose/sessions"),
            home.join(".cursor/chats"),
            home.join(".cline"),
            home.join(".kiro"),
        ];
        for path in expected {
            assert!(
                dirs.iter().any(|d| d == &path),
                "watch list should include {} (got {:?})",
                path.display(),
                dirs,
            );
        }
    }

    #[test]
    fn watch_dirs_for_home_enumerates_hermes_profiles_only_when_directories() {
        // Hermes supports per-profile session directories under
        // `<HERMES_HOME>/profiles/<profile>/sessions`. Only entries that are
        // actually directories should be subscribed to — stray files in
        // `profiles/` must be skipped.
        let temp = tempdir().unwrap();
        let home = temp.path();
        let hermes = home.join(".hermes");
        let profiles = hermes.join("profiles");
        fs::create_dir_all(profiles.join("prod")).unwrap();
        fs::create_dir_all(profiles.join("staging")).unwrap();
        // A regular file in the same parent — must be ignored.
        fs::write(profiles.join("README.md"), "ignored").unwrap();

        let dirs = watch_dirs_for_home(home);
        let expected_prod = profiles.join("prod/sessions");
        let expected_staging = profiles.join("staging/sessions");
        let readme_subpath = profiles.join("README.md/sessions");

        assert!(dirs.iter().any(|d| d == &expected_prod));
        assert!(dirs.iter().any(|d| d == &expected_staging));
        assert!(
            !dirs.iter().any(|d| d == &readme_subpath),
            "non-directory entries should not become watch paths"
        );
    }
}
