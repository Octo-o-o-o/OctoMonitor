fn main() {
    ensure_placeholder_sidecar();
    tauri_build::build()
}

fn ensure_placeholder_sidecar() {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, path::Path};

    let path = Path::new("bundled/octomonitor-server");
    if path.exists() {
        return;
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        path,
        b"#!/bin/sh\nprintf 'octomonitor-server sidecar has not been prepared for bundling\\n' >&2\nexit 1\n",
    );
    #[cfg(unix)]
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}
