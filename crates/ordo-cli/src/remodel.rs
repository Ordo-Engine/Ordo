//! Ask a decision model again: the request side of `ordo impact --model`.
//!
//! The gateway's decision log (`ordo-server --decide-log-requests`) keeps each
//! request as it was sent to the model. Before switching models, every logged
//! request is sent to the new model through any System One compatible API
//! (`POST <upstream>/v1/systemone`), and the new answers replace the old ones in
//! the rule input. The facts stay exactly as they were logged.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value as Json;

/// Requests in flight at once.
const PARALLEL: usize = 8;

/// Default upstream when neither `--upstream` nor `ORDO_DECIDE_UPSTREAM` is set.
pub const DEFAULT_UPSTREAM: &str = "https://openrouter.ai/api";

/// One gateway call from the decision log that can be asked again.
pub struct LoggedCall {
    /// 1-based line number in the log
    pub line: usize,
    /// The model that answered (`typesafe/jev-1.13-20260917`)
    pub model: String,
    /// The request as sent upstream
    pub request: Json,
    /// The rule input: facts plus the answers under `answers_field`
    pub input: Json,
    pub answers_field: String,
}

/// What the log held for one ruleset.
pub struct Log {
    pub calls: Vec<LoggedCall>,
    /// Decisions for this ruleset logged without their request
    pub without_request: usize,
}

/// Read the decision-log lines for `ruleset` that carry a rule input.
pub fn read_log(path: &Path, ruleset: &str) -> Result<Log> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read decision log {}", path.display()))?;
    let mut log = Log {
        calls: Vec::new(),
        without_request: 0,
    };
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Json = serde_json::from_str(line)
            .with_context(|| format!("{}:{}: invalid JSON", path.display(), i + 1))?;
        if v.get("ruleset").and_then(Json::as_str) != Some(ruleset) {
            continue;
        }
        let Some(input) = v.get("input").filter(|i| i.is_object()) else {
            continue;
        };
        let Some(request) = v
            .get("request")
            .filter(|r| r.get("state").is_some_and(|s| !s.is_null()))
        else {
            log.without_request += 1;
            continue;
        };
        log.calls.push(LoggedCall {
            line: i + 1,
            model: v
                .get("model")
                .and_then(Json::as_str)
                .unwrap_or("?")
                .to_string(),
            request: request.clone(),
            input: input.clone(),
            answers_field: v
                .get("answers_field")
                .and_then(Json::as_str)
                .unwrap_or("jev")
                .to_string(),
        });
    }
    Ok(log)
}

/// The new model's reply to one logged request.
pub struct Reply {
    /// The model id the upstream reports (may carry a dated version)
    pub model: String,
    pub answers: Json,
}

/// System One client for `--model`.
pub struct Asker {
    client: reqwest::blocking::Client,
    url: String,
    key: Option<String>,
    model: String,
}

impl Asker {
    pub fn new(upstream: &str, model: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        // Same variables the gateway and the Jev SDKs read.
        let key = [
            "ORDO_DECIDE_API_KEY",
            "OPENROUTER_API_KEY",
            "TYPESAFE_API_KEY",
        ]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
        Ok(Self {
            client,
            url: format!("{}/v1/systemone", upstream.trim_end_matches('/')),
            key,
            model: model.to_string(),
        })
    }

    /// Send one logged request with the model swapped, retrying once on a
    /// network error, a 429 or a 5xx.
    pub fn ask(&self, request: &Json) -> std::result::Result<Reply, String> {
        match self.ask_once(request) {
            Err((_, true)) => {
                std::thread::sleep(Duration::from_secs(2));
                self.ask_once(request).map_err(|(m, _)| m)
            }
            other => other.map_err(|(m, _)| m),
        }
    }

    /// `Err((message, worth_retrying))`
    fn ask_once(&self, request: &Json) -> std::result::Result<Reply, (String, bool)> {
        let mut body = request.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.remove("ordo");
            obj.insert("model".into(), Json::String(self.model.clone()));
        }
        let mut req = self.client.post(&self.url).json(&body);
        if let Some(key) = &self.key {
            req = req.bearer_auth(key);
        }
        let res = req
            .send()
            .map_err(|e| (format!("{}: {e}", self.url), true))?;
        let status = res.status();
        let text = res.text().map_err(|e| (e.to_string(), true))?;
        if !status.is_success() {
            let detail = serde_json::from_str::<Json>(&text)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message")
                        .and_then(Json::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| text.chars().take(200).collect());
            let retry = status.as_u16() == 429 || status.is_server_error();
            return Err((format!("{} {status}: {detail}", self.model), retry));
        }
        let v: Json =
            serde_json::from_str(&text).map_err(|e| (format!("invalid reply: {e}"), false))?;
        let answers = v
            .get("answers")
            .filter(|a| a.is_object())
            .cloned()
            .ok_or_else(|| ("reply has no answers".to_string(), false))?;
        Ok(Reply {
            model: v
                .get("model")
                .and_then(Json::as_str)
                .unwrap_or(&self.model)
                .to_string(),
            answers,
        })
    }

    /// Ask every request, a few at a time; replies come back in the same order.
    pub fn ask_all(&self, requests: &[&Json]) -> Vec<std::result::Result<Reply, String>> {
        let next = AtomicUsize::new(0);
        let out: Mutex<Vec<Option<std::result::Result<Reply, String>>>> =
            Mutex::new((0..requests.len()).map(|_| None).collect());
        std::thread::scope(|s| {
            for _ in 0..PARALLEL.min(requests.len()) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(request) = requests.get(i) else {
                        break;
                    };
                    let r = self.ask(request);
                    out.lock().unwrap()[i] = Some(r);
                });
            }
        });
        out.into_inner()
            .unwrap()
            .into_iter()
            .map(|r| r.unwrap_or_else(|| Err("not asked".to_string())))
            .collect()
    }
}

/// The parts of an answer a rule reads: the verdict fields, without the
/// probability tables and legends that change on every call.
pub fn verdict(answers: &Json) -> Json {
    let Json::Object(map) = answers else {
        return answers.clone();
    };
    Json::Object(
        map.iter()
            .map(|(k, v)| {
                let v = match v {
                    Json::Object(a) => Json::Object(
                        a.iter()
                            .filter(|(f, _)| {
                                !matches!(f.as_str(), "probabilities" | "legend" | "type")
                            })
                            .map(|(f, x)| (f.clone(), x.clone()))
                            .collect(),
                    ),
                    other => other.clone(),
                };
                (k.clone(), v)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_only_decisions_for_the_ruleset_and_counts_missing_requests() {
        let path =
            std::env::temp_dir().join(format!("ordo-remodel-log-{}.jsonl", std::process::id()));
        std::fs::write(
            &path,
            [
                r#"{"ruleset":"refund-routing","model":"jev-1.13.0","input":{"jev":{"a":1},"order":{}},"answers_field":"jev","request":{"model":"jev-1.13","state":"hi","questions":{}}}"#,
                r#"{"ruleset":"refund-routing","model":"jev-1.13.0","input":{"jev":{"a":2}}}"#,
                r#"{"ruleset":"other","input":{"x":1},"request":{"state":"x"}}"#,
                r#"{"model":"jev-1.13.0","answers":{},"input":null,"request":{"state":"no rules"}}"#,
                r#"{"ruleset":"refund-routing","model":"jev-1.13.0","input":{"signals":{"a":3}},"answers_field":"signals","request":{"state":"yo"}}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        let log = read_log(&path, "refund-routing").unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(log.calls.len(), 2);
        assert_eq!(log.without_request, 1);
        assert_eq!(log.calls[0].line, 1);
        assert_eq!(log.calls[0].answers_field, "jev");
        assert_eq!(log.calls[1].line, 5);
        assert_eq!(log.calls[1].answers_field, "signals");
    }

    #[test]
    fn verdict_drops_probability_tables() {
        let a = json!({
            "intent": {"type": "choice", "choice": "refund", "confidence": 0.97,
                       "probabilities": {"refund": 0.98}},
            "urgency": {"type": "score", "score": 0.4, "legend": {"0": "x"}},
        });
        assert_eq!(
            verdict(&a),
            json!({"intent": {"choice": "refund", "confidence": 0.97}, "urgency": {"score": 0.4}})
        );
    }
}
