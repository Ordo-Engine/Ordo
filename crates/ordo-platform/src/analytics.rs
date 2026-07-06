//! Rule-execution analytics: turn stored cumulative-counter snapshots into
//! time-bucketed rates for Studio.
//!
//! Engines report **cumulative** counters (totals since start, reset to 0 on
//! restart). We diff consecutive snapshots of the same (server, ruleset) —
//! reset-safe — attribute each delta to the bucket of the later snapshot, and
//! sum across servers.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiResult, PlatformError},
    models::Claims,
    store::ExecutionSnapshot,
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    /// Lookback window, e.g. `1h`, `24h`, `7d` (default `24h`).
    #[serde(default)]
    pub range: Option<String>,
    /// Bucket width in seconds (default derived from range).
    #[serde(default)]
    pub bucket: Option<i64>,
    /// Restrict to a single ruleset.
    #[serde(default)]
    pub ruleset: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Totals {
    pub calls: f64,
    pub errors: f64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub by_code: BTreeMap<String, f64>,
}

#[derive(Debug, Serialize)]
pub struct SeriesPoint {
    pub ts: String,
    pub calls: f64,
    pub errors: f64,
    pub by_code: BTreeMap<String, f64>,
}

#[derive(Debug, Serialize)]
pub struct RulesetSummary {
    pub ruleset: String,
    pub calls: f64,
    pub errors: f64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub totals: Totals,
    pub series: Vec<SeriesPoint>,
    pub rulesets: Vec<RulesetSummary>,
    pub bucket_seconds: i64,
    pub from: String,
    pub to: String,
}

/// Reset-safe delta between a later cumulative value and the previous one.
fn delta(cur: f64, prev: f64) -> f64 {
    if cur >= prev {
        cur - prev
    } else {
        // Counter reset (engine restart) — the current value is itself the delta.
        cur
    }
}

#[derive(Default, Clone)]
struct Bucket {
    calls: f64,
    errors: f64,
    by_code: BTreeMap<String, f64>,
}

#[derive(Default, Clone)]
struct RulesetAcc {
    calls: f64,
    errors: f64,
    duration_count: f64,
    duration_sum: f64,
    by_code: BTreeMap<String, f64>,
}

/// Aggregate ordered snapshots (by server_id, ruleset, captured_at asc) into a
/// bucketed analytics response over `[from, to]`.
pub fn aggregate(
    snapshots: &[ExecutionSnapshot],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    bucket_seconds: i64,
) -> AnalyticsResponse {
    let bucket_seconds = bucket_seconds.max(1);
    let mut buckets: BTreeMap<i64, Bucket> = BTreeMap::new();
    let mut per_ruleset: BTreeMap<String, RulesetAcc> = BTreeMap::new();

    // Walk consecutive snapshots within each (server_id, ruleset) group. The
    // input is ordered so same-group snapshots are adjacent and time-ascending.
    let mut i = 0;
    while i < snapshots.len() {
        let mut j = i + 1;
        while j < snapshots.len()
            && snapshots[j].server_id == snapshots[i].server_id
            && snapshots[j].ruleset == snapshots[i].ruleset
        {
            let prev = &snapshots[j - 1];
            let cur = &snapshots[j];

            let d_success = delta(cur.exec_success, prev.exec_success);
            let d_error = delta(cur.exec_error, prev.exec_error);
            let d_calls = d_success + d_error;
            let d_dcount = delta(cur.duration_count, prev.duration_count);
            let d_dsum = delta(cur.duration_sum_seconds, prev.duration_sum_seconds);

            // Bucket keyed by the later snapshot's time.
            let secs = cur.captured_at.timestamp();
            let bts = secs - secs.rem_euclid(bucket_seconds);
            let b = buckets.entry(bts).or_default();
            b.calls += d_calls;
            b.errors += d_error;

            let acc = per_ruleset.entry(cur.ruleset.clone()).or_default();
            acc.calls += d_calls;
            acc.errors += d_error;
            acc.duration_count += d_dcount;
            acc.duration_sum += d_dsum;

            for (code, cur_v) in &cur.terminal {
                let prev_v = prev.terminal.get(code).copied().unwrap_or(0.0);
                let dv = delta(*cur_v, prev_v);
                if dv != 0.0 {
                    *b.by_code.entry(code.clone()).or_default() += dv;
                    *acc.by_code.entry(code.clone()).or_default() += dv;
                }
            }
            j += 1;
        }
        i = j;
    }

    // Totals.
    let mut totals = Totals::default();
    let mut total_dcount = 0.0;
    let mut total_dsum = 0.0;
    let mut rulesets: Vec<RulesetSummary> = Vec::new();
    for (name, acc) in per_ruleset {
        totals.calls += acc.calls;
        totals.errors += acc.errors;
        total_dcount += acc.duration_count;
        total_dsum += acc.duration_sum;
        for (code, v) in &acc.by_code {
            *totals.by_code.entry(code.clone()).or_default() += v;
        }
        rulesets.push(RulesetSummary {
            ruleset: name,
            calls: acc.calls,
            errors: acc.errors,
            error_rate: rate(acc.errors, acc.calls),
            avg_latency_ms: avg_ms(acc.duration_sum, acc.duration_count),
        });
    }
    totals.error_rate = rate(totals.errors, totals.calls);
    totals.avg_latency_ms = avg_ms(total_dsum, total_dcount);
    rulesets.sort_by(|a, b| b.calls.total_cmp(&a.calls));

    let series = buckets
        .into_iter()
        .map(|(bts, b)| SeriesPoint {
            ts: DateTime::from_timestamp(bts, 0)
                .unwrap_or(from)
                .to_rfc3339(),
            calls: b.calls,
            errors: b.errors,
            by_code: b.by_code,
        })
        .collect();

    AnalyticsResponse {
        totals,
        series,
        rulesets,
        bucket_seconds,
        from: from.to_rfc3339(),
        to: to.to_rfc3339(),
    }
}

fn rate(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        part / whole
    } else {
        0.0
    }
}

fn avg_ms(sum_seconds: f64, count: f64) -> f64 {
    if count > 0.0 {
        sum_seconds / count * 1000.0
    } else {
        0.0
    }
}

/// Parse `1h` / `24h` / `7d` / `30m` into a Duration (default 24h; clamped 5m–90d).
fn parse_range(range: Option<&str>) -> Duration {
    let default = Duration::hours(24);
    let Some(s) = range else { return default };
    let s = s.trim();
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: i64 = num.parse().unwrap_or(24);
    let d = match unit {
        "m" => Duration::minutes(n),
        "h" | "" => Duration::hours(n),
        "d" => Duration::days(n),
        _ => default,
    };
    d.clamp(Duration::minutes(5), Duration::days(90))
}

/// Default bucket width so a window has a sane number of points. The fine tier
/// (≤15min → 15s) matches the engine's default reporting interval; a client can
/// pass an explicit `bucket` (e.g. 6) to match a finer engine interval.
fn default_bucket_seconds(window: Duration) -> i64 {
    let secs = window.num_seconds().max(15);
    if secs <= 15 * 60 {
        15 // ≤15min → 15-second buckets (the fine "recent" view)
    } else if secs <= 2 * 3600 {
        60 // ≤2h → 1-minute buckets
    } else if secs <= 2 * 86400 {
        3600 // ≤2d → hourly
    } else {
        86400 // else daily
    }
}

/// GET /api/v1/orgs/:oid/projects/:pid/analytics
pub async fn get_project_analytics(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((_org_id, project_id)): Path<(String, String)>,
    Query(q): Query<AnalyticsQuery>,
) -> ApiResult<Json<AnalyticsResponse>> {
    let (org_id, _role) =
        crate::catalog::resolve_project(&state, &project_id, &claims.sub, None).await?;

    let window = parse_range(q.range.as_deref());
    let to = Utc::now();
    let from = to - window;
    let bucket = q.bucket.unwrap_or_else(|| default_bucket_seconds(window));

    // Scope to the project's rulesets (or a single requested one within them).
    let mut names = state
        .store
        .list_project_ruleset_names(&project_id)
        .await
        .map_err(PlatformError::Internal)?;
    if let Some(rs) = &q.ruleset {
        names.retain(|n| n == rs);
    }

    let snapshots = state
        .store
        .fetch_execution_snapshots(&org_id, Some(&names), from, to)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(aggregate(&snapshots, from, to, bucket)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn snap(
        server: &str,
        ruleset: &str,
        secs: i64,
        success: f64,
        error: f64,
        terminal: &[(&str, f64)],
        dcount: f64,
        dsum: f64,
    ) -> ExecutionSnapshot {
        ExecutionSnapshot {
            server_id: server.into(),
            ruleset: ruleset.into(),
            captured_at: DateTime::from_timestamp(secs, 0).unwrap(),
            exec_success: success,
            exec_error: error,
            terminal: terminal.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            duration_count: dcount,
            duration_sum_seconds: dsum,
        }
    }

    #[test]
    fn diffs_cumulative_counters_into_deltas() {
        // One server, one ruleset, three snapshots: 10 → 15 → 22 success.
        let snaps = vec![
            snap(
                "s1",
                "loan",
                0,
                10.0,
                1.0,
                &[("APPROVED", 8.0), ("REJECTED", 2.0)],
                11.0,
                0.011,
            ),
            snap(
                "s1",
                "loan",
                60,
                15.0,
                2.0,
                &[("APPROVED", 12.0), ("REJECTED", 3.0)],
                17.0,
                0.017,
            ),
            snap(
                "s1",
                "loan",
                120,
                22.0,
                2.0,
                &[("APPROVED", 19.0), ("REJECTED", 3.0)],
                24.0,
                0.024,
            ),
        ];
        let from = DateTime::from_timestamp(0, 0).unwrap();
        let to = DateTime::from_timestamp(200, 0).unwrap();
        let r = aggregate(&snaps, from, to, 60);

        // deltas: calls (5+1)+(7+0)=13; errors 1+0=1.
        assert_eq!(r.totals.calls, 13.0);
        assert_eq!(r.totals.errors, 1.0);
        // by_code APPROVED: (12-8)+(19-12)=11; REJECTED: (3-2)+(3-3)=1.
        assert_eq!(r.totals.by_code["APPROVED"], 11.0);
        assert_eq!(r.totals.by_code["REJECTED"], 1.0);
        // latency: Δsum=0.013, Δcount=13 → avg 1ms.
        assert!((r.totals.avg_latency_ms - 1.0).abs() < 1e-6);
        assert_eq!(r.rulesets.len(), 1);
        assert!(r.series.len() >= 2);
    }

    #[test]
    fn reset_safe_when_engine_restarts() {
        // success drops 20 → 3 (restart). The 3 counts as the delta, not -17.
        let snaps = vec![
            snap("s1", "loan", 0, 20.0, 0.0, &[], 20.0, 0.02),
            snap("s1", "loan", 60, 3.0, 0.0, &[], 3.0, 0.003),
        ];
        let from = DateTime::from_timestamp(0, 0).unwrap();
        let to = DateTime::from_timestamp(120, 0).unwrap();
        let r = aggregate(&snaps, from, to, 60);
        assert_eq!(r.totals.calls, 3.0);
        assert!(r.totals.calls >= 0.0);
    }

    #[test]
    fn separate_servers_do_not_cross_diff() {
        // Two servers, each 5→8. Total delta = 3+3 = 6, not cross-diffed.
        let snaps = vec![
            snap("s1", "loan", 0, 5.0, 0.0, &[], 5.0, 0.0),
            snap("s1", "loan", 60, 8.0, 0.0, &[], 8.0, 0.0),
            snap("s2", "loan", 0, 5.0, 0.0, &[], 5.0, 0.0),
            snap("s2", "loan", 60, 8.0, 0.0, &[], 8.0, 0.0),
        ];
        let from = DateTime::from_timestamp(0, 0).unwrap();
        let to = DateTime::from_timestamp(120, 0).unwrap();
        let r = aggregate(&snaps, from, to, 60);
        assert_eq!(r.totals.calls, 6.0);
    }

    #[test]
    fn parse_range_units() {
        assert_eq!(parse_range(Some("1h")), Duration::hours(1));
        assert_eq!(parse_range(Some("7d")), Duration::days(7));
        assert_eq!(parse_range(Some("30m")), Duration::minutes(30));
        assert_eq!(parse_range(None), Duration::hours(24));
    }
}
