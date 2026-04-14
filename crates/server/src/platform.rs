use std::{env, ffi::OsString, path::PathBuf};

fn home_drive_path() -> Option<OsString> {
    let (Some(drive), Some(path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) else {
        return None;
    };
    let mut combined = drive;
    combined.push(path);
    Some(combined)
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(home_drive_path)
        .map(PathBuf::from)
}

pub fn home_relative_path(relative: &str) -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(relative)
}

pub fn expand_home_path(raw: &str) -> PathBuf {
    let rest = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"));
    match (home_dir(), rest) {
        (Some(home), Some(rest)) => home.join(rest),
        _ => PathBuf::from(raw),
    }
}

pub fn last_path_component(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
}

pub fn shorten_path(path: &str) -> String {
    let Some(home) = home_dir() else {
        return path.into();
    };
    let home = home.to_string_lossy();
    let candidates = [
        home.to_string(),
        home.replace('\\', "/"),
        home.replace('/', "\\"),
    ];

    for candidate in candidates {
        if let Some(rest) = path.strip_prefix(&candidate) {
            return if rest.is_empty() {
                "~".into()
            } else {
                format!("~{rest}")
            };
        }
    }

    path.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_path_component_handles_both_separator_styles() {
        assert_eq!(last_path_component("/tmp/octomonitor"), Some("octomonitor"));
        assert_eq!(
            last_path_component(r"C:\Users\demo\octomonitor"),
            Some("octomonitor")
        );
    }

    #[test]
    fn expand_home_path_leaves_regular_paths_untouched() {
        assert_eq!(
            expand_home_path("/tmp/octomonitor"),
            PathBuf::from("/tmp/octomonitor")
        );
        assert_eq!(
            expand_home_path(r"C:\Users\demo\octomonitor"),
            PathBuf::from(r"C:\Users\demo\octomonitor")
        );
    }
}
