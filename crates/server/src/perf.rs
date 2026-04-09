use std::sync::OnceLock;
use std::time::Instant;

use octomonitor_core::BootstrapPayload;

fn parse_env_flag(raw: &str) -> bool {
    matches!(raw, "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
}

fn perf_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("OCTOMONITOR_PERF_LOG")
            .map(|raw| parse_env_flag(raw.trim()))
            .unwrap_or(false)
    })
}

pub fn log_elapsed(operation: &'static str, started_at: Instant) {
    if !perf_logging_enabled() {
        return;
    }

    tracing::info!(
        target: "octomonitor::perf",
        operation,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "perf span complete"
    );
}

pub fn log_elapsed_with_details(
    operation: &'static str,
    started_at: Instant,
    details: impl FnOnce() -> String,
) {
    if !perf_logging_enabled() {
        return;
    }

    tracing::info!(
        target: "octomonitor::perf",
        operation,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        details = %details(),
        "perf span complete"
    );
}

pub fn log_bootstrap_payload(context: &'static str, payload: &BootstrapPayload) {
    if !perf_logging_enabled() {
        return;
    }

    match serde_json::to_vec(payload) {
        Ok(bytes) => tracing::info!(
            target: "octomonitor::perf",
            context,
            payload_bytes = bytes.len(),
            runs = payload.runs.len(),
            commits = payload.commits.len(),
            usage_buckets = payload.usage_buckets.len(),
            completions = payload.recent_completions.len(),
            workflow_runs = payload.workflow_runs.len(),
            "bootstrap payload sampled"
        ),
        Err(error) => tracing::warn!(
            target: "octomonitor::perf",
            context,
            %error,
            "failed to serialize bootstrap payload for perf sampling"
        ),
    }
}

pub fn log_ingest_event(source: &'static str, run_id: &str) {
    if !perf_logging_enabled() {
        return;
    }

    tracing::info!(
        target: "octomonitor::perf",
        source,
        run_id,
        "ingest event received"
    );
}

pub fn log_probe_wake(reason: &'static str) {
    if !perf_logging_enabled() {
        return;
    }

    tracing::info!(
        target: "octomonitor::perf",
        reason,
        "probe wake requested"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_flag_accepts_common_truthy_values() {
        assert!(parse_env_flag("1"));
        assert!(parse_env_flag("true"));
        assert!(parse_env_flag("YES"));
        assert!(parse_env_flag("on"));
        assert!(!parse_env_flag("0"));
        assert!(!parse_env_flag("false"));
    }
}
