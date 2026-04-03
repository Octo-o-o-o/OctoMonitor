#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

struct ServerState(Mutex<Option<Child>>);
struct DesktopBootState(Mutex<Option<DesktopBootIssue>>);

const SERVER_ADDR: &str = "127.0.0.1:46321";
const SERVER_HEALTH_PATH: &str = "/api/health";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopBootIssue {
    title: String,
    message: String,
}

struct SpawnResult {
    child: Option<Child>,
    launch_error: Option<String>,
}

enum ChildStatus {
    Running,
    Missing,
    Exited(String),
    Unknown(String),
}

fn find_server_binary() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidates = [
        // 1) Same directory — works for target/{debug,release}
        dir.join("octomonitor-server"),
        // 2) Preferred bundle location after resource remap
        dir.parent()?.join("Resources").join("octomonitor-server"),
        // 3) Backward-compatible fallbacks for older bundle layouts
        dir.parent()?
            .join("Resources")
            .join("target")
            .join("release")
            .join("octomonitor-server"),
        dir.parent()?
            .join("Resources")
            .join("_up_")
            .join("_up_")
            .join("_up_")
            .join("target")
            .join("release")
            .join("octomonitor-server"),
    ];

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn spawn_server() -> SpawnResult {
    // Try to find the pre-built server binary first
    if let Some(binary) = find_server_binary() {
        return match Command::new(&binary)
            .env("OCTOMONITOR_NO_OPEN", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => SpawnResult {
                child: Some(child),
                launch_error: None,
            },
            Err(error) => SpawnResult {
                child: None,
                launch_error: Some(format!(
                    "Desktop shell found `{}` but could not launch it: {error}",
                    binary.display()
                )),
            },
        };
    }

    // Debug-only fallback: use cargo run (requires source tree + toolchain)
    #[cfg(debug_assertions)]
    {
        let workspace_root = env::current_exe()
            .ok()
            .and_then(|exe| {
                // Navigate from target/debug up to workspace root
                exe.parent()?.parent()?.parent().map(|p| p.to_path_buf())
            })
            .unwrap_or_else(|| PathBuf::from("."));

        match Command::new("cargo")
            .args(["run", "-p", "octomonitor-server"])
            .current_dir(workspace_root)
            .env("OCTOMONITOR_NO_OPEN", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => SpawnResult {
                child: Some(child),
                launch_error: None,
            },
            Err(error) => SpawnResult {
                child: None,
                launch_error: Some(format!(
                    "Desktop shell could not run `cargo run -p octomonitor-server`: {error}"
                )),
            },
        }
    }

    #[cfg(not(debug_assertions))]
    {
        eprintln!("octomonitor-server binary not found; server will not start");
        SpawnResult {
            child: None,
            launch_error: Some(
                "Bundled `octomonitor-server` binary was not found and no local server was started."
                    .into(),
            ),
        }
    }
}

fn check_server_health() -> bool {
    let Ok(addr) = SERVER_ADDR.parse::<SocketAddr>() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(600)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(600)));

    if stream
        .write_all(
            format!(
                "GET {SERVER_HEALTH_PATH} HTTP/1.1\r\nHost: {SERVER_ADDR}\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .is_err()
    {
        return false;
    }

    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }

    response.contains("\"status\":\"ok\"") || response.contains("\"status\": \"ok\"")
}

fn inspect_server_process(app: &AppHandle) -> ChildStatus {
    let Some(state) = app.try_state::<ServerState>() else {
        return ChildStatus::Missing;
    };
    let Ok(mut guard) = state.0.lock() else {
        return ChildStatus::Unknown(
            "Desktop shell could not lock the server process state.".into(),
        );
    };
    let Some(process) = guard.as_mut() else {
        return ChildStatus::Missing;
    };

    match process.try_wait() {
        Ok(Some(status)) => {
            guard.take();
            ChildStatus::Exited(match status.code() {
                Some(code) => format!("exit code {code}"),
                None => "terminated by signal".into(),
            })
        }
        Ok(None) => ChildStatus::Running,
        Err(error) => ChildStatus::Unknown(format!(
            "Desktop shell could not inspect the server process state: {error}"
        )),
    }
}

fn push_boot_issue(app: &AppHandle, issue: Option<DesktopBootIssue>) {
    if let Some(state) = app.try_state::<DesktopBootState>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = issue.clone();
        }
    }

    let payload = serde_json::to_string(&issue).unwrap_or_else(|_| "null".into());
    let script = format!(
        "window.__OCTOMONITOR_DESKTOP_BOOT__ = {payload}; window.dispatchEvent(new CustomEvent('octomonitor:desktop-boot-status', {{ detail: {payload} }}));"
    );
    for window in app.webview_windows().values() {
        let _ = window.eval(script.as_str());
    }
}

fn monitor_server_readiness(app: AppHandle, launch_error: Option<String>) {
    thread::spawn(move || {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        let mut timeout_reported = false;

        loop {
            if check_server_health() {
                if timeout_reported {
                    push_boot_issue(&app, None);
                }
                break;
            }

            match inspect_server_process(&app) {
                ChildStatus::Running => {}
                ChildStatus::Missing => {
                    if timeout_reported {
                        break;
                    }
                }
                ChildStatus::Exited(detail) => {
                    if check_server_health() {
                        break;
                    }
                    let launch_hint = launch_error.as_deref().unwrap_or(
                        "Another process may already be using port 46321, or the bundled server exited before becoming healthy.",
                    );
                    push_boot_issue(
                        &app,
                        Some(DesktopBootIssue {
                            title: "Local server failed to start".into(),
                            message: format!(
                                "The bundled OctoMonitor server exited before becoming ready ({detail}). {launch_hint}"
                            ),
                        }),
                    );
                    break;
                }
                ChildStatus::Unknown(message) => {
                    push_boot_issue(
                        &app,
                        Some(DesktopBootIssue {
                            title: "Local server status is unknown".into(),
                            message,
                        }),
                    );
                    break;
                }
            }

            if !timeout_reported && Instant::now() >= deadline {
                let issue = DesktopBootIssue {
                    title: if launch_error.is_some() {
                        "Desktop server unavailable".into()
                    } else {
                        "Local server did not become ready".into()
                    },
                    message: launch_error.as_deref().unwrap_or(
                        "Desktop shell could not confirm that http://127.0.0.1:46321/api/health became ready within 8 seconds. OctoMonitor is still warming up in the background; if this banner persists, retry or start `cargo run -p octomonitor-server` manually.",
                    ).into(),
                };
                push_boot_issue(&app, Some(issue));
                timeout_reported = true;

                if matches!(inspect_server_process(&app), ChildStatus::Missing) {
                    break;
                }
            }

            thread::sleep(STARTUP_POLL_INTERVAL);
        }
    });
}

fn stop_server(app: &AppHandle) {
    if let Some(state) = app.try_state::<ServerState>() {
        if let Ok(mut guard) = state.0.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
            }
        }
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let spawn_result = spawn_server();
            app.manage(ServerState(Mutex::new(spawn_result.child)));
            app.manage(DesktopBootState(Mutex::new(None)));
            monitor_server_readiness(app.handle().clone(), spawn_result.launch_error);
            Ok(())
        })
        .on_page_load(|window, _| {
            let issue = window
                .app_handle()
                .try_state::<DesktopBootState>()
                .and_then(|state| state.0.lock().ok().and_then(|issue| issue.clone()));
            let payload = serde_json::to_string(&issue).unwrap_or_else(|_| "null".into());
            let script = format!(
                "window.__OCTOMONITOR_DESKTOP_BOOT__ = {payload}; window.dispatchEvent(new CustomEvent('octomonitor:desktop-boot-status', {{ detail: {payload} }}));"
            );
            let _ = window.eval(script);
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                stop_server(window.app_handle());
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
