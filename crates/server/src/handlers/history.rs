use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use octomonitor_core::{CommitHistoryPayload, UsageHistoryPayload};
use serde::Deserialize;

use crate::{
    probe::{build_commit_history_from_runs, build_usage_history_from_runs, collect_history_runs},
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub from: String,
    pub to: String,
}

fn parse_history_range(query: &HistoryQuery) -> Result<(DateTime<Utc>, DateTime<Utc>), StatusCode> {
    let mut from = DateTime::parse_from_rfc3339(&query.from)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .with_timezone(&Utc);
    let mut to = DateTime::parse_from_rfc3339(&query.to)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .with_timezone(&Utc);

    if from > to {
        std::mem::swap(&mut from, &mut to);
    }

    let now = Utc::now();
    if from > now {
        from = now;
    }
    if to > now {
        to = now;
    }

    let max_span = Duration::days(3650);
    if to - from > max_span {
        from = to - max_span;
    }

    Ok((from, to))
}

pub async fn get_usage_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<UsageHistoryPayload>, StatusCode> {
    let (from, to) = parse_history_range(&query)?;
    let runs = collect_history_runs(&state).await;
    let pricing = state.pricing.clone();
    let payload = tokio::task::spawn_blocking(move || {
        build_usage_history_from_runs(&pricing, runs, from, to)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(payload))
}

pub async fn get_commit_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<CommitHistoryPayload>, StatusCode> {
    let (from, to) = parse_history_range(&query)?;
    let runs = collect_history_runs(&state).await;
    let pricing = state.pricing.clone();
    let payload = tokio::task::spawn_blocking(move || {
        build_commit_history_from_runs(&pricing, runs, from, to)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(payload))
}
