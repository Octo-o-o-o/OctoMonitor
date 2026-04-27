#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{
    menu::{AboutMetadata, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Manager, Runtime,
};

/// Shared handle to the server child process.
/// Both the Tauri window-destroy handler and the post-run cleanup use this.
type SharedChild = Arc<Mutex<Option<Child>>>;

struct ServerState(SharedChild);
struct DesktopBootState(Mutex<Option<DesktopBootIssue>>);

const SERVER_ADDR: &str = "127.0.0.1:46321";
const SERVER_HEALTH_PATH: &str = "/api/health";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(150);
const DESKTOP_BOOT_EVENT: &str = "octomonitor:desktop-boot-status";
const DESKTOP_MENU_ACTION_EVENT: &str = "octomonitor:desktop-menu-action";

const MENU_APP_PREFERENCES: &str = "app.preferences";
const MENU_VIEW_ZOOM_IN: &str = "view.zoom_in";
const MENU_VIEW_ZOOM_OUT: &str = "view.zoom_out";
const MENU_VIEW_ZOOM_RESET: &str = "view.zoom_reset";
const MENU_HELP_KEYBOARD_SHORTCUTS: &str = "help.keyboard_shortcuts";

const ACTION_OPEN_SETTINGS: &str = "open-settings";
const ACTION_TOGGLE_SHORTCUTS: &str = "toggle-shortcuts";
const ACTION_ZOOM_IN: &str = "zoom-in";
const ACTION_ZOOM_OUT: &str = "zoom-out";
const ACTION_ZOOM_RESET: &str = "zoom-reset";

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

fn dispatch_window_script(app: &AppHandle, script: &str) {
    for window in app.webview_windows().values() {
        let _ = window.eval(script);
    }
}

fn to_json_or<T: Serialize>(value: &T, fallback: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| fallback.into())
}

fn emit_menu_action(app: &AppHandle, action: &str) {
    let detail = to_json_or(&serde_json::json!({ "action": action }), "null");
    dispatch_window_script(
        app,
        &format!(
            "window.dispatchEvent(new CustomEvent('{DESKTOP_MENU_ACTION_EVENT}', {{ detail: {detail} }}));"
        ),
    );
}

/// Build the script that seeds `window.__OCTOMONITOR_DESKTOP_BOOT__` and then
/// dispatches the boot-status custom event so the web UI sees both the cached
/// value and a fresh event notification.
fn boot_issue_script(issue: &Option<DesktopBootIssue>) -> String {
    let payload = to_json_or(issue, "null");
    format!(
        "window.__OCTOMONITOR_DESKTOP_BOOT__ = {payload}; \
         window.dispatchEvent(new CustomEvent('{DESKTOP_BOOT_EVENT}', {{ detail: {payload} }}));"
    )
}

fn build_app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    #[cfg(target_os = "macos")]
    let about_metadata = {
        let pkg_info = app.package_info();
        let config = app.config();
        AboutMetadata {
            name: Some(pkg_info.name.clone()),
            version: Some(pkg_info.version.to_string()),
            copyright: config.bundle.copyright.clone(),
            authors: config
                .bundle
                .publisher
                .clone()
                .map(|publisher| vec![publisher]),
            ..Default::default()
        }
    };

    #[cfg(target_os = "macos")]
    let preferences = MenuItemBuilder::with_id(MENU_APP_PREFERENCES, "Preferences...")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;
    let zoom_in = MenuItemBuilder::with_id(MENU_VIEW_ZOOM_IN, "Zoom In")
        .accelerator("CmdOrCtrl+=")
        .build(app)?;
    let zoom_out = MenuItemBuilder::with_id(MENU_VIEW_ZOOM_OUT, "Zoom Out")
        .accelerator("CmdOrCtrl+-")
        .build(app)?;
    let zoom_reset = MenuItemBuilder::with_id(MENU_VIEW_ZOOM_RESET, "Actual Size")
        .accelerator("CmdOrCtrl+0")
        .build(app)?;
    let keyboard_shortcuts =
        MenuItemBuilder::with_id(MENU_HELP_KEYBOARD_SHORTCUTS, "Keyboard Shortcuts").build(app)?;

    let mut menu = MenuBuilder::new(app);

    #[cfg(target_os = "macos")]
    {
        let app_menu = SubmenuBuilder::new(app, app.package_info().name.clone())
            .about(Some(about_metadata))
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .separator()
            .item(&preferences)
            .separator()
            .quit()
            .build()?;
        menu = menu.item(&app_menu);
    }

    let file_menu = SubmenuBuilder::new(app, "File").close_window().build()?;
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let mut view_menu = SubmenuBuilder::new(app, "View")
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&zoom_reset);
    #[cfg(target_os = "macos")]
    {
        view_menu = view_menu.separator().fullscreen();
    }
    let view_menu = view_menu.build()?;
    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;
    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&keyboard_shortcuts)
        .build()?;

    menu.item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}

fn find_server_binary() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let resources = dir.parent()?.join("Resources");
    let candidates = [
        // Same directory — works for target/{debug,release}
        dir.join("octomonitor-server"),
        // Preferred bundle location after resource remap
        resources.join("octomonitor-server"),
        // Backward-compatible fallbacks for older bundle layouts
        resources.join("target/release/octomonitor-server"),
        resources.join("_up_/_up_/_up_/target/release/octomonitor-server"),
    ];

    candidates.into_iter().find(|candidate| candidate.exists())
}

/// Build a Command for the server, setting up a new process group on Unix
/// so that kill_child can terminate the entire tree.
fn new_server_command(program: &std::ffi::OsStr) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Start the child in its own process group (pgid = child pid).
        // This lets us kill the whole group later.
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
    }
    cmd
}

fn spawn_with(mut command: Command, on_error: impl FnOnce(std::io::Error) -> String) -> SpawnResult {
    match command
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
            launch_error: Some(on_error(error)),
        },
    }
}

fn spawn_server() -> SpawnResult {
    // Try to find the pre-built server binary first.
    if let Some(binary) = find_server_binary() {
        let display = binary.display().to_string();
        return spawn_with(new_server_command(binary.as_os_str()), |error| {
            format!("Desktop shell found `{display}` but could not launch it: {error}")
        });
    }

    // Debug-only fallback: use `cargo run` from the workspace root. This
    // requires the source tree + toolchain, so it is never compiled into
    // release builds.
    #[cfg(debug_assertions)]
    {
        let workspace_root = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent()?.parent()?.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));

        let mut cmd = new_server_command(std::ffi::OsStr::new("cargo"));
        cmd.args(["run", "-p", "octomonitor-server"])
            .current_dir(workspace_root);
        spawn_with(cmd, |error| {
            format!("Desktop shell could not run `cargo run -p octomonitor-server`: {error}")
        })
    }

    #[cfg(not(debug_assertions))]
    {
        eprintln!("octomonitor-server binary not found; server will not start");
        SpawnResult {
            child: None,
            launch_error: Some(
                "Bundled `octomonitor-server` binary was not found and no local server was started.".into(),
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

    let request = format!(
        "GET {SERVER_HEALTH_PATH} HTTP/1.1\r\nHost: {SERVER_ADDR}\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
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
            let detail = status
                .code()
                .map(|code| format!("exit code {code}"))
                .unwrap_or_else(|| "terminated by signal".into());
            ChildStatus::Exited(detail)
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

    dispatch_window_script(app, &boot_issue_script(&issue));
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
                let (title, message) = match launch_error.as_deref() {
                    Some(err) => ("Desktop server unavailable".into(), err.to_owned()),
                    None => (
                        "Local server did not become ready".into(),
                        "Desktop shell could not confirm that http://127.0.0.1:46321/api/health became ready within 8 seconds. OctoMonitor is still warming up in the background; if this banner persists, retry or start `cargo run -p octomonitor-server` manually.".into(),
                    ),
                };
                push_boot_issue(&app, Some(DesktopBootIssue { title, message }));
                timeout_reported = true;

                if matches!(inspect_server_process(&app), ChildStatus::Missing) {
                    break;
                }
            }

            thread::sleep(STARTUP_POLL_INTERVAL);
        }
    });
}

/// Terminate the server child process and its entire process group.
fn kill_child(child: &mut Child) {
    #[cfg(unix)]
    {
        // Kill the entire process group (the child was spawned with setpgid).
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGTERM);
        }
        // Give the group a moment to exit gracefully, then force-kill.
        thread::sleep(Duration::from_millis(200));
        let _ = child.try_wait();
        let _ = child.kill(); // SIGKILL if still alive
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait(); // reap to avoid zombie
}

/// Take the child out of the shared state and kill it.
fn stop_server_shared(shared: &SharedChild) {
    let Ok(mut guard) = shared.lock() else { return };
    if let Some(mut child) = guard.take() {
        kill_child(&mut child);
    }
}

fn main() {
    let spawn_result = spawn_server();
    let shared_child: SharedChild = Arc::new(Mutex::new(spawn_result.child));
    let shared_for_setup = shared_child.clone();

    tauri::Builder::default()
        .menu(build_app_menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_APP_PREFERENCES => emit_menu_action(app, ACTION_OPEN_SETTINGS),
            MENU_VIEW_ZOOM_IN => emit_menu_action(app, ACTION_ZOOM_IN),
            MENU_VIEW_ZOOM_OUT => emit_menu_action(app, ACTION_ZOOM_OUT),
            MENU_VIEW_ZOOM_RESET => emit_menu_action(app, ACTION_ZOOM_RESET),
            MENU_HELP_KEYBOARD_SHORTCUTS => emit_menu_action(app, ACTION_TOGGLE_SHORTCUTS),
            _ => {}
        })
        .setup(move |app| {
            app.manage(ServerState(shared_for_setup));
            app.manage(DesktopBootState(Mutex::new(None)));
            monitor_server_readiness(app.handle().clone(), spawn_result.launch_error);
            Ok(())
        })
        .on_page_load(|window, _| {
            let issue = window
                .app_handle()
                .try_state::<DesktopBootState>()
                .and_then(|state| state.0.lock().ok()?.clone());
            let _ = window.eval(boot_issue_script(&issue));
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Some(state) = window.app_handle().try_state::<ServerState>() {
                    stop_server_shared(&state.0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");

    // Safety net: if the Destroyed event never fired (e.g. Cmd+Q on macOS),
    // the shared Arc still holds the child. Kill it now.
    stop_server_shared(&shared_child);
}
