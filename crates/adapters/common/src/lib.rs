use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDescriptor {
    pub tool: &'static str,
    pub preferred_mode: &'static str,
    pub fallback_mode: &'static str,
    pub confidence: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProbeResult {
    pub command: String,
    pub success: bool,
    pub stdout_snippet: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProbeResult {
    pub path: String,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JsonlCursor {
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub struct JsonlDelta {
    pub lines: Vec<String>,
    pub reset: bool,
}

pub fn run_command_probe(cmd: &str, args: &[&str]) -> CommandProbeResult {
    let command = format!("{} {}", cmd, args.join(" "));
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            CommandProbeResult {
                command,
                success,
                stdout_snippet: snippet(&stdout),
                error: if success { None } else { snippet(&stderr) },
            }
        }
        Err(e) => CommandProbeResult {
            command,
            success: false,
            stdout_snippet: None,
            error: Some(e.to_string()),
        },
    }
}

fn snippet(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.chars().take(200).collect())
    }
}

pub fn probe_file(path: &Path) -> FileProbeResult {
    let meta = fs::metadata(path).ok();
    FileProbeResult {
        path: path.display().to_string(),
        exists: meta.is_some(),
        size_bytes: meta.as_ref().map(fs::Metadata::len),
        modified_at: meta
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<Utc>::from(t).to_rfc3339()),
    }
}

pub fn read_jsonl_delta(path: &Path, cursor: &mut JsonlCursor) -> io::Result<JsonlDelta> {
    let mut file = fs::File::open(path)?;
    let reset = file.metadata()?.len() < cursor.offset;
    if reset {
        cursor.offset = 0;
    }

    file.seek(SeekFrom::Start(cursor.offset))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        line.truncate(line.trim_end_matches(['\n', '\r']).len());
        lines.push(std::mem::take(&mut line));
    }
    cursor.offset = reader.stream_position()?;

    Ok(JsonlDelta { lines, reset })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretScanFinding {
    pub pattern: &'static str,
    pub line_number: usize,
}

pub fn scan_text_for_secret_patterns(text: &str) -> Vec<SecretScanFinding> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, line)| secret_pattern_for_line(line).map(|pattern| (idx, pattern)))
        .map(|(idx, pattern)| SecretScanFinding {
            pattern,
            line_number: idx + 1,
        })
        .collect()
}

fn secret_pattern_for_line(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.contains("-----BEGIN") && trimmed.contains("PRIVATE KEY-----") {
        return Some("private-key-block");
    }
    if contains_token_prefix(trimmed, "sk-ant-") {
        return Some("anthropic-key");
    }
    if contains_token_prefix(trimmed, "sk-proj-") || contains_token_prefix(trimmed, "sk-") {
        return Some("openai-key");
    }
    if contains_token_prefix(trimmed, "ghp_") || contains_token_prefix(trimmed, "github_pat_") {
        return Some("github-token");
    }
    if contains_token_prefix(trimmed, "AIza") {
        return Some("google-api-key");
    }
    if contains_aws_access_key(trimmed) {
        return Some("aws-access-key");
    }
    if looks_like_secret_assignment(&lower) {
        return Some("secret-assignment");
    }
    None
}

fn contains_token_prefix(line: &str, prefix: &str) -> bool {
    line.find(prefix).is_some_and(|idx| {
        line[idx + prefix.len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
            .count()
            >= 16
    })
}

fn contains_aws_access_key(line: &str) -> bool {
    line.find("AKIA").is_some_and(|idx| {
        line[idx..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .count()
            >= 20
    })
}

fn looks_like_secret_assignment(lower_line: &str) -> bool {
    const SECRET_NAMES: &[&str] = &[
        "api_key",
        "apikey",
        "access_token",
        "refresh_token",
        "auth_token",
        "client_secret",
        "private_key",
        "secret_key",
        "bearer_token",
        "password",
    ];

    let Some(separator_idx) = lower_line.find(['=', ':']) else {
        return false;
    };
    let key = lower_line[..separator_idx]
        .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
    let value = lower_line[separator_idx + 1..]
        .trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace() || c == ',');
    if value.is_empty()
        || value.contains("placeholder")
        || value.contains("redacted")
        || value.contains("fixture")
        || value.contains("example")
        || value.contains('<')
    {
        return false;
    }
    value.len() >= 12 && SECRET_NAMES.iter().any(|name| key.ends_with(name))
}

pub fn path_has_sensitive_component(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        component_name_is_sensitive(&name)
    })
}

fn component_name_is_sensitive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let normalized = lower.replace(['-', '.'], "_");
    lower == ".env"
        || lower.ends_with(".env")
        || lower.starts_with(".env.")
        || normalized.contains("credential")
        || normalized.contains("oauth")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("access_token")
        || normalized.contains("refresh_token")
        || normalized.contains("auth_token")
        || normalized.contains("provider_secret")
        || normalized.contains("private_key")
        || normalized.contains("secret")
}

/// Mask a sensitive value, showing only the first and last 4 characters.
/// Values shorter than `min_visible` are fully masked.
pub fn mask_value(value: &str, min_visible: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= min_visible {
        return "****".to_string();
    }
    let prefix: String = chars[..4].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}…{suffix}")
}

/// Resolve a home-relative directory, falling back to HOME / USERPROFILE or "."
pub fn resolve_home_dir(relative: &str) -> PathBuf {
    home_dir()
        .map(|home| home.join(relative))
        .unwrap_or_else(|| PathBuf::from(".").join(relative))
}

fn home_drive_path() -> Option<OsString> {
    let mut combined = env::var_os("HOMEDRIVE")?;
    combined.push(env::var_os("HOMEPATH")?);
    Some(combined)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(home_drive_path)
        .map(PathBuf::from)
}

/// Resolve a directory by checking environment variables in order, falling back
/// to the given home-relative path. Empty env values are ignored.
pub fn resolve_env_or_home(env_vars: &[&str], home_relative: &str) -> PathBuf {
    env_vars
        .iter()
        .filter_map(|name| env::var(name).ok())
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_home_dir(home_relative))
}

/// Capitalize the first character of a string.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Truncate a string to at most `max_chars` characters (counting chars, not bytes).
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Metadata signature used to invalidate per-file caches.
pub fn file_signature(path: &Path) -> Option<(u64, Option<SystemTime>)> {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

/// Scan `dir` for direct file entries and return the most recently modified
/// file name. Directories and unreadable entries are ignored. If `extension`
/// is `Some`, only files with that extension are considered.
pub fn latest_file_name(dir: &Path, extension: Option<&str>) -> Option<String> {
    let mut newest: Option<(SystemTime, String)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        if let Some(ext) = extension {
            if entry.path().extension().is_none_or(|e| e != ext) {
                continue;
            }
        }
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
            newest = Some((modified, name));
        }
    }
    newest.map(|(_, name)| name)
}

/// Format a standard 5-field cron expression (`min hour dom mon dow`) as a
/// human-readable schedule. Falls back to `"<expr> (<tz>)"` when the
/// expression uses wildcards we don't render specially.
pub fn format_cron_expr(expr: &str, tz: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() < 5 {
        return format!("{expr} ({tz})");
    }
    let (min, hour, _dom, _mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);

    if hour == "*" || min == "*" {
        return format!("{expr} ({tz})");
    }
    let time_str = format!("{hour:0>2}:{min:0>2}");

    if dow == "*" {
        return format!("Daily {time_str}");
    }

    let dow_str = dow
        .split(',')
        .map(|d| match d {
            "0" => "Sun",
            "1" => "Mon",
            "2" => "Tue",
            "3" => "Wed",
            "4" => "Thu",
            "5" => "Fri",
            "6" => "Sat",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{dow_str} {time_str}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalize_handles_empty_and_unicode() {
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("telegram"), "Telegram");
        assert_eq!(capitalize("é"), "É");
    }

    #[test]
    fn truncate_chars_counts_chars_not_bytes() {
        assert_eq!(truncate_chars("héllo", 3), "hél");
        assert_eq!(truncate_chars("abc", 10), "abc");
    }

    #[test]
    fn format_cron_expr_formats_common_patterns() {
        assert_eq!(format_cron_expr("0 9 * * *", "UTC"), "Daily 09:00");
        assert_eq!(
            format_cron_expr("30 17 * * 1,3,5", "UTC"),
            "Mon,Wed,Fri 17:30"
        );
        assert_eq!(format_cron_expr("* * * * *", "UTC"), "* * * * * (UTC)");
        assert_eq!(format_cron_expr("bad", "UTC"), "bad (UTC)");
    }

    #[test]
    fn latest_file_name_skips_dirs_and_wrong_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.jsonl"), "a").expect("a");
        std::fs::create_dir(dir.path().join("sub")).expect("sub");
        std::fs::write(dir.path().join("b.txt"), "b").expect("b");

        let latest = latest_file_name(dir.path(), Some("jsonl"));
        assert_eq!(latest.as_deref(), Some("a.jsonl"));
    }

    #[test]
    fn read_jsonl_delta_reads_only_new_lines_and_resets_on_truncate() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("session.jsonl");
        std::fs::write(&path, "{\"a\":1}\n{\"a\":2}\n").expect("initial file");

        let mut cursor = JsonlCursor::default();
        let first = read_jsonl_delta(&path, &mut cursor).expect("first read");
        assert!(!first.reset);
        assert_eq!(first.lines.len(), 2);

        std::fs::write(&path, "{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n").expect("append file");
        let second = read_jsonl_delta(&path, &mut cursor).expect("second read");
        assert!(!second.reset);
        assert_eq!(second.lines, vec![r#"{"a":3}"#]);

        std::fs::write(&path, "{\"b\":1}\n").expect("truncate file");
        let third = read_jsonl_delta(&path, &mut cursor).expect("third read");
        assert!(third.reset);
        assert_eq!(third.lines, vec![r#"{"b":1}"#]);
    }

    #[test]
    fn secret_scan_detects_common_key_shapes() {
        let findings =
            scan_text_for_secret_patterns("OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz\n");
        assert_eq!(findings[0].pattern, "openai-key");

        let findings = scan_text_for_secret_patterns("refresh_token: live-token-value-12345\n");
        assert_eq!(findings[0].pattern, "secret-assignment");
    }

    #[test]
    fn secret_scan_allows_fixture_placeholders() {
        assert!(scan_text_for_secret_patterns(
            "denied path: ~/.tool/credentials/provider.json\napi_key: fixture-placeholder\n"
        )
        .is_empty());
    }

    #[test]
    fn sensitive_path_matcher_catches_credential_names() {
        assert!(path_has_sensitive_component(Path::new(
            "/Users/demo/.tool/credentials/oauth.json"
        )));
        assert!(path_has_sensitive_component(Path::new(
            "/Users/demo/project/.env.local"
        )));
        assert!(!path_has_sensitive_component(Path::new(
            "/Users/demo/project/session.jsonl"
        )));
    }
}

#[cfg(test)]
mod agent_fixture_contract_tests {
    use super::*;
    use serde_json::Value;
    use std::{collections::HashMap, collections::HashSet, path::PathBuf};

    const REQUIRED_TOOLS: &[&str] = &[
        "codebuddy",
        "continue-cn",
        "cline",
        "qwen",
        "kiro",
        "kimi",
        "goose",
        "cursor",
        "opencode",
    ];
    const REQUIRED_CASE_FILES: &[&str] = &[
        "evidence_lock.json",
        "schema_fingerprint.json",
        "golden_sessions.json",
        "commands.sh",
        "README.md",
    ];

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn fixture_root() -> PathBuf {
        workspace_root().join("fixtures/agents")
    }

    fn json_file(path: &Path) -> Value {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("invalid json {}: {err}", path.display()))
    }

    fn collect_text_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir)
            .unwrap_or_else(|err| panic!("failed to read dir {}: {err}", dir.display()))
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                collect_text_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    #[test]
    fn agent_fixture_cases_follow_contract() {
        let root = fixture_root();
        assert!(root.is_dir(), "{} must exist", root.display());

        let mut polarity: HashMap<String, (bool, bool)> = HashMap::new();
        let mut stable_declared: Vec<String> = Vec::new();

        for tool_entry in fs::read_dir(&root)
            .expect("fixtures/agents readable")
            .flatten()
        {
            let tool_path = tool_entry.path();
            if !tool_path.is_dir() {
                continue;
            }
            let tool_id = tool_entry.file_name().to_string_lossy().into_owned();
            polarity.entry(tool_id.clone()).or_default();

            for version_entry in fs::read_dir(&tool_path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", tool_path.display()))
                .flatten()
            {
                let version_path = version_entry.path();
                if !version_path.is_dir() {
                    continue;
                }
                let fixture_version = version_entry.file_name().to_string_lossy().into_owned();

                for case_entry in fs::read_dir(&version_path)
                    .unwrap_or_else(|err| {
                        panic!("failed to read {}: {err}", version_path.display())
                    })
                    .flatten()
                {
                    let case_path = case_entry.path();
                    if !case_path.is_dir() {
                        continue;
                    }
                    let case_id = case_entry.file_name().to_string_lossy().into_owned();
                    for required in REQUIRED_CASE_FILES {
                        assert!(
                            case_path.join(required).is_file(),
                            "{} missing {required}",
                            case_path.display()
                        );
                    }

                    let evidence = json_file(&case_path.join("evidence_lock.json"));
                    assert_eq!(evidence["tool_id"].as_str(), Some(tool_id.as_str()));
                    assert_eq!(
                        evidence["fixture_version"].as_str(),
                        Some(fixture_version.as_str())
                    );
                    assert_eq!(evidence["case_id"].as_str(), Some(case_id.as_str()));
                    assert!(evidence["source_url"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("https://")));
                    assert!(evidence["source_path"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(evidence["evidence_level"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(evidence["local_command_used"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(evidence["denied_paths_observed"].is_array());

                    let support_level = evidence["target_support_level"].as_str().unwrap_or("");
                    if matches!(support_level, "stable" | "monitored") {
                        stable_declared.push(format!("{tool_id}/{fixture_version}/{case_id}"));
                    }

                    let schema = json_file(&case_path.join("schema_fingerprint.json"));
                    assert_eq!(schema["tool_id"].as_str(), Some(tool_id.as_str()));
                    assert_eq!(schema["case_id"].as_str(), Some(case_id.as_str()));
                    assert!(schema["format"].as_str().is_some_and(|s| !s.is_empty()));
                    assert!(matches!(
                        schema["schema_confidence"].as_str(),
                        Some("high" | "medium" | "low" | "unsupported")
                    ));

                    let golden = json_file(&case_path.join("golden_sessions.json"));
                    assert!(
                        golden.is_array(),
                        "{} golden_sessions.json must be an array",
                        case_path.display()
                    );

                    let commands = fs::read_to_string(case_path.join("commands.sh"))
                        .expect("commands.sh readable");
                    assert!(commands.contains("set -eu"));
                    assert!(commands.contains("octomonitor-fixture:"));

                    if case_id.starts_with("positive") {
                        polarity.get_mut(&tool_id).expect("tool polarity").0 = true;
                    }
                    if case_id.starts_with("negative") {
                        polarity.get_mut(&tool_id).expect("tool polarity").1 = true;
                    }
                }
            }
        }

        let found_tools: HashSet<&str> = polarity.keys().map(String::as_str).collect();
        for required in REQUIRED_TOOLS {
            assert!(
                found_tools.contains(required),
                "missing fixture tool {required}"
            );
            let (has_positive, has_negative) = polarity[*required];
            assert!(has_positive, "{required} needs a positive fixture case");
            assert!(has_negative, "{required} needs a negative fixture case");
        }

        for stable_case in stable_declared {
            let tool = stable_case.split('/').next().unwrap_or_default();
            let (has_positive, has_negative) = polarity[tool];
            assert!(
                has_positive && has_negative,
                "{stable_case} cannot be marked stable without positive and negative fixtures"
            );
        }
    }

    #[test]
    fn agent_fixtures_do_not_contain_secret_patterns() {
        let root = fixture_root();
        let mut files = Vec::new();
        collect_text_files(&root, &mut files);

        for path in files {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let findings = scan_text_for_secret_patterns(&text);
            assert!(
                findings.is_empty(),
                "{} contains secret-looking fixture text: {:?}",
                path.display(),
                findings
            );
        }
    }
}
