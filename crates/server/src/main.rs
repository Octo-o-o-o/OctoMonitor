mod commits;
mod config;
mod handlers;
mod network;
mod perf;
mod platform;
mod pricing;
mod probe;
mod remote_access;
mod state;
mod static_files;
#[cfg(test)]
mod test_support;
mod watcher;
mod workflows;

use std::net::{IpAddr, SocketAddr};

use axum::{
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use config::{load_config, save_config};
use handlers::{
    bootstrap, config as config_handler, daily_summary, history, ingest, inspect, installer,
    pairing, remote, stream, workflows as wf,
};
use pricing::PricingStore;
use probe::{empty_bootstrap, spawn_derive_refresh, spawn_probe_refresh};
use remote_access::spawn_remote_server;
use state::AppState;

const SERVER_PORT: u16 = 46321;
const SNAPSHOT_WINDOW_MIGRATION_VERSION: u8 = 2;

fn local_allowed_origins() -> Vec<HeaderValue> {
    [
        "http://127.0.0.1:4173",
        "http://localhost:4173",
        "http://127.0.0.1:46321",
        "http://localhost:46321",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "tauri://localhost",
    ]
    .into_iter()
    .map(HeaderValue::from_static)
    .collect()
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(local_allowed_origins())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT])
}

pub(crate) fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/api/bootstrap", get(bootstrap::get_bootstrap))
        .route("/api/health", get(bootstrap::health))
        .route("/api/history/usage", get(history::get_usage_history))
        .route("/api/history/commits", get(history::get_commit_history))
        .route("/api/runs/{run_id}/inspect", get(inspect::get_run_inspect))
        .route(
            "/api/config",
            get(config_handler::get_config).patch(config_handler::patch_config),
        )
        .route(
            "/api/daily-summary/generate",
            post(daily_summary::generate_daily_summary),
        )
        .route("/api/installer/detect", get(installer::installer_detect))
        .route("/api/installer/doctor", get(installer::installer_doctor))
        .route(
            "/api/installer/install-plan",
            post(installer::installer_install_plan),
        )
        .route("/api/installer/install", post(installer::installer_install))
        .route(
            "/api/installer/rollback",
            post(installer::installer_rollback),
        )
        .route("/api/pair/request", post(pairing::pair_request))
        .route("/api/pair/approve/{token}", post(pairing::pair_approve))
        .route("/api/pair/revoke/{token}", post(pairing::pair_revoke))
        .route(
            "/api/remote/access",
            get(remote::get_remote_access).patch(remote::patch_remote_access),
        )
        .route("/api/remote/devices", get(remote::list_remote_devices))
        .route("/api/remote/pairings", post(remote::create_remote_pairing))
        .route(
            "/api/remote/devices/{device_id}",
            axum::routing::delete(remote::revoke_remote_device),
        )
        .route(
            "/api/ingest/claude/statusline",
            post(ingest::ingest_claude_statusline),
        )
        .route("/api/ingest/claude/hook", post(ingest::ingest_claude_hook))
        .route("/api/ingest/codex/hook", post(ingest::ingest_codex_hook))
        .route("/api/stream", get(stream::stream))
        .route("/api/workflows", get(wf::list_defs).post(wf::create_def))
        .route(
            "/api/workflows/{id}",
            get(wf::get_def).put(wf::update_def).delete(wf::delete_def),
        )
        .route("/api/workflows/{id}/runs", post(wf::create_run))
        .route("/api/workflow-runs", get(wf::list_runs))
        .route("/api/workflow-runs/{id}", get(wf::get_run))
        .route("/api/workflow-runs/{id}/cancel", post(wf::cancel_run))
        .route("/api/workflow-runs/{id}/mode", post(wf::change_mode))
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/link",
            post(wf::link_run),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/unlink",
            post(wf::unlink_run),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/complete",
            post(wf::complete_step),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/fail",
            post(wf::fail_step),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/skip",
            post(wf::skip_step),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/retry",
            post(wf::retry_step),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/candidates",
            get(wf::get_candidates),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/preview",
            get(wf::get_launch_preview),
        )
        .route(
            "/api/workflow-runs/{id}/steps/{step_id}/approve",
            post(wf::approve_step),
        )
        .layer(build_cors_layer())
        .with_state(state)
        .fallback(static_files::static_handler)
}

fn apply_saved_config(initial: &mut octomonitor_core::BootstrapPayload) -> bool {
    let Some(saved) = load_config() else {
        return false;
    };
    if let Some(v) = saved.companion_enabled {
        initial.config.companion_enabled = v;
    }
    let Some(days) = saved.history_days else {
        return false;
    };
    let clamped = days.clamp(1, 180);
    let needs_migration =
        saved.version.unwrap_or(0) < SNAPSHOT_WINDOW_MIGRATION_VERSION && clamped == 7;
    if needs_migration {
        initial.config.history_days = 30;
    } else {
        initial.config.history_days = clamped;
    }
    needs_migration
}

fn parse_bind_ip(raw: Option<&str>) -> anyhow::Result<IpAddr> {
    raw.unwrap_or("127.0.0.1")
        .parse::<IpAddr>()
        .map_err(|e| anyhow::anyhow!("Invalid OCTOMONITOR_BIND_ADDR: {e}"))
}

fn resolve_bind_addr() -> anyhow::Result<SocketAddr> {
    let bind_ip = parse_bind_ip(std::env::var("OCTOMONITOR_BIND_ADDR").ok().as_deref())?;
    Ok(SocketAddr::from((bind_ip, SERVER_PORT)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let pricing = PricingStore::new();
    pricing.spawn_refresh();

    let mut initial = empty_bootstrap();
    let migrated = apply_saved_config(&mut initial);
    if migrated {
        if let Err(error) = save_config(&initial.config) {
            tracing::warn!("Failed to persist migrated config: {error}");
        }
    }

    let wf_store =
        workflows::store::WorkflowStore::new(workflows::store::WorkflowStore::default_base_dir())
            .expect("Failed to initialize workflow store");
    let wf_coordinator = workflows::coordinator::WorkflowCoordinator::new(wf_store);

    // Load workflow summaries into bootstrap
    initial.workflow_runs = wf_coordinator.get_summary_list();

    let state = AppState::new(initial, pricing, wf_coordinator);

    let app = build_app(state.clone());

    let addr = resolve_bind_addr()?;
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "Port {SERVER_PORT} is already in use — is another OctoMonitor instance running?\n\
                 Stop the other instance or inspect port {SERVER_PORT} with your OS networking tools."
            )
        } else {
            anyhow::anyhow!("Failed to bind to {}: {}", addr, e)
        }
    })?;

    let open_browser = std::env::var("OCTOMONITOR_NO_OPEN").is_err();
    tracing::info!("OctoMonitor server listening on http://{}", addr);
    spawn_probe_refresh(state.clone());
    spawn_derive_refresh(state.clone());
    watcher::spawn_fs_watcher(state.clone());
    spawn_remote_server(state.clone());
    if open_browser {
        let _ = open::that(format!("http://{}", addr));
    }

    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_bind_addr_defaults_to_localhost() {
        let addr = SocketAddr::from((
            parse_bind_ip(None).expect("bind ip should parse"),
            SERVER_PORT,
        ));

        assert_eq!(addr, SocketAddr::from(([127, 0, 0, 1], SERVER_PORT)));
    }

    #[test]
    fn resolve_bind_addr_respects_env_override() {
        let addr = SocketAddr::from((
            parse_bind_ip(Some("0.0.0.0")).expect("bind ip should parse"),
            SERVER_PORT,
        ));

        assert_eq!(addr, SocketAddr::from(([0, 0, 0, 0], SERVER_PORT)));
    }

    #[test]
    fn local_allowed_origins_include_loopback_and_tauri() {
        let origins = local_allowed_origins()
            .into_iter()
            .map(|value| value.to_str().expect("origin should be utf-8").to_string())
            .collect::<Vec<_>>();

        assert!(origins.contains(&"http://127.0.0.1:4173".to_string()));
        assert!(origins.contains(&"http://localhost:4173".to_string()));
        assert!(origins.contains(&"https://tauri.localhost".to_string()));
        assert!(origins.contains(&"tauri://localhost".to_string()));
    }
}
