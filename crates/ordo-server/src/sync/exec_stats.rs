//! Snapshot the engine's cumulative execution counters into a structured,
//! per-ruleset form for reporting to the control plane.
//!
//! Reads the Prometheus global registry via `prometheus::gather()` (the same
//! source `metrics::encode_metrics` renders to text) rather than parsing text.

use std::collections::BTreeMap;

use super::event::RulesetExecStat;

/// Build a per-ruleset snapshot of the cumulative execution counters:
/// `ordo_executions_total{ruleset,result}`, `ordo_terminal_results_total`
/// `{ruleset,result_code}`, and `ordo_execution_duration_seconds{ruleset}`
/// (histogram count + sum). Values are cumulative since engine start.
pub fn build_execution_stats() -> Vec<RulesetExecStat> {
    from_families(&prometheus::gather())
}

fn label<'a>(m: &'a prometheus::proto::Metric, key: &str) -> Option<&'a str> {
    m.get_label()
        .iter()
        .find(|l| l.get_name() == key)
        .map(|l| l.get_value())
}

fn from_families(families: &[prometheus::proto::MetricFamily]) -> Vec<RulesetExecStat> {
    let mut by_ruleset: BTreeMap<String, RulesetExecStat> = BTreeMap::new();

    for fam in families {
        match fam.get_name() {
            "ordo_executions_total" => {
                for m in fam.get_metric() {
                    let Some(rs) = label(m, "ruleset") else {
                        continue;
                    };
                    let result = label(m, "result").unwrap_or_default();
                    let v = m.get_counter().get_value();
                    let e = by_ruleset.entry(rs.to_string()).or_default();
                    e.ruleset = rs.to_string();
                    match result {
                        "success" => e.exec_success += v,
                        "error" => e.exec_error += v,
                        _ => {}
                    }
                }
            }
            "ordo_terminal_results_total" => {
                for m in fam.get_metric() {
                    let Some(rs) = label(m, "ruleset") else {
                        continue;
                    };
                    let code = label(m, "result_code").unwrap_or_default().to_string();
                    let v = m.get_counter().get_value();
                    let e = by_ruleset.entry(rs.to_string()).or_default();
                    e.ruleset = rs.to_string();
                    *e.terminal.entry(code).or_default() += v;
                }
            }
            "ordo_execution_duration_seconds" => {
                for m in fam.get_metric() {
                    let Some(rs) = label(m, "ruleset") else {
                        continue;
                    };
                    let h = m.get_histogram();
                    let e = by_ruleset.entry(rs.to_string()).or_default();
                    e.ruleset = rs.to_string();
                    e.duration_count += h.get_sample_count() as f64;
                    e.duration_sum_seconds += h.get_sample_sum();
                }
            }
            _ => {}
        }
    }

    by_ruleset.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics;

    #[test]
    fn build_groups_counters_by_ruleset() {
        // Record a few executions into the global registry.
        metrics::record_execution_success("t_loan", 0.001);
        metrics::record_execution_success("t_loan", 0.002);
        metrics::record_execution_error("t_loan", 0.003);
        metrics::record_terminal_result("t_loan", "APPROVED");
        metrics::record_terminal_result("t_loan", "APPROVED");
        metrics::record_terminal_result("t_loan", "REJECTED");

        let stats = build_execution_stats();
        let loan = stats
            .iter()
            .find(|s| s.ruleset == "t_loan")
            .expect("t_loan present");

        // The global registry may accumulate across tests; assert monotonic minimums.
        assert!(loan.exec_success >= 2.0, "success={}", loan.exec_success);
        assert!(loan.exec_error >= 1.0, "error={}", loan.exec_error);
        assert!(loan.duration_count >= 3.0, "count={}", loan.duration_count);
        assert!(loan.duration_sum_seconds > 0.0);
        assert!(*loan.terminal.get("APPROVED").unwrap_or(&0.0) >= 2.0);
        assert!(*loan.terminal.get("REJECTED").unwrap_or(&0.0) >= 1.0);
    }
}
