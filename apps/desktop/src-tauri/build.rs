use std::{ffi::OsStr, fs, path::Path};

fn main() {
    ensure_placeholder_frontend_dist();
    ensure_placeholder_sidecar();
    tauri_build::build()
}

fn ensure_placeholder_frontend_dist() {
    let dist_dir = Path::new("../../web/dist");
    if !dist_dir.exists() {
        let _ = fs::create_dir_all(dist_dir);
    }

    ensure_html_shell(&dist_dir.join("index.html"), "OctoMonitor");
    ensure_referenced_assets(dist_dir);
}

fn ensure_placeholder_sidecar() {
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
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    }
}

fn ensure_html_shell(path: &Path, title: &str) {
    if path.exists() {
        return;
    }
    let _ = fs::write(
        path,
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{title}</title></head><body><div id=\"root\"></div></body></html>"
        ),
    );
}

fn ensure_referenced_assets(dist_dir: &Path) {
    let Ok(entries) = fs::read_dir(dist_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "html") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        for reference in extract_asset_references(&contents) {
            let relative = reference.trim_start_matches('/');
            if relative.is_empty() {
                continue;
            }
            let asset_path = dist_dir.join(relative);
            if asset_path.exists() {
                continue;
            }
            if let Some(parent) = asset_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let placeholder = match asset_path.extension().and_then(OsStr::to_str) {
                Some("js") => {
                    "console.warn('OctoMonitor placeholder asset loaded during cargo test');\n"
                }
                Some("css") => "html,body{margin:0;padding:0;}\n",
                _ => "",
            };
            let _ = fs::write(asset_path, placeholder);
        }
    }
}

fn extract_asset_references(html: &str) -> Vec<&str> {
    let mut refs = Vec::new();
    for marker in ["src=\"", "href=\""] {
        let mut remainder = html;
        while let Some(index) = remainder.find(marker) {
            remainder = &remainder[index + marker.len()..];
            let Some(value_end) = remainder.find('"') else {
                break;
            };
            let value = &remainder[..value_end];
            if !value.starts_with("http://")
                && !value.starts_with("https://")
                && !value.starts_with("data:")
                && !value.starts_with('#')
            {
                refs.push(value);
            }
            remainder = &remainder[value_end + 1..];
        }
    }
    refs
}
