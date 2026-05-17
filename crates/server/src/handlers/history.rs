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
    from = from.min(now);
    to = to.min(now);

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

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };

    use super::*;
    use crate::test_support::ServerTestHarness;

    fn q(from: &str, to: &str) -> HistoryQuery {
        HistoryQuery {
            from: from.to_string(),
            to: to.to_string(),
        }
    }

    #[test]
    fn parse_history_range_rejects_invalid_rfc3339() {
        let err = parse_history_range(&q("not-a-date", "2026-04-01T00:00:00Z"))
            .expect_err("invalid `from` should fail");
        assert_eq!(err, StatusCode::BAD_REQUEST);

        let err = parse_history_range(&q("2026-04-01T00:00:00Z", "still-not-a-date"))
            .expect_err("invalid `to` should fail");
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn parse_history_range_swaps_when_from_after_to() {
        let (from, to) = parse_history_range(&q(
            "2026-04-10T00:00:00Z",
            "2026-04-01T00:00:00Z",
        ))
        .expect("swap should succeed");
        assert!(from < to, "swapped pair should satisfy from < to");
        assert_eq!(from.to_rfc3339(), "2026-04-01T00:00:00+00:00");
        assert_eq!(to.to_rfc3339(), "2026-04-10T00:00:00+00:00");
    }

    #[test]
    fn parse_history_range_clamps_future_bounds_to_now() {
        // Use a date far in the future so the clamp is observable regardless
        // of wall-clock drift between assertion and parse.
        let (from, to) = parse_history_range(&q(
            "2099-01-01T00:00:00Z",
            "2099-12-31T00:00:00Z",
        ))
        .expect("future range should parse");
        let now = Utc::now();
        assert!(from <= now, "from should be clamped to <= now");
        assert!(to <= now, "to should be clamped to <= now");
    }

    #[test]
    fn parse_history_range_caps_span_to_ten_years() {
        let (from, to) = parse_history_range(&q(
            "1990-01-01T00:00:00Z",
            "2026-04-01T00:00:00Z",
        ))
        .expect("wide range should parse");
        let span = to - from;
        assert!(
            span <= Duration::days(3650),
            "span must be clamped to <= 3650 days, got {} days",
            span.num_days()
        );
        // The narrower bound stays close to the original `to`.
        assert_eq!(to.to_rfc3339(), "2026-04-01T00:00:00+00:00");
    }

    #[tokio::test]
    async fn usage_history_returns_400_on_invalid_query() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/history/usage?from=bad&to=2026-04-01T00:00:00Z")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn commit_history_returns_400_on_invalid_query() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri("/api/history/commits?from=2026-04-01T00:00:00Z&to=bad")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn usage_history_returns_200_on_well_formed_query() {
        let harness = ServerTestHarness::new();
        let response = harness
            .request(
                Request::builder()
                    .uri(
                        "/api/history/usage\
                         ?from=2026-04-01T00:00:00Z\
                         &to=2026-04-30T00:00:00Z",
                    )
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
