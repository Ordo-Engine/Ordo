use super::*;

pub(super) fn build_release_content_diff(
    baseline: Option<&JsonValue>,
    target: &JsonValue,
    baseline_version: Option<&str>,
) -> ReleaseContentDiffSummary {
    let before_steps = baseline.map(extract_steps).unwrap_or_default();
    let after_steps = extract_steps(target);
    let before_groups = baseline.map(extract_groups).unwrap_or_default();
    let after_groups = extract_groups(target);

    let before_ids: BTreeSet<_> = before_steps.keys().cloned().collect();
    let after_ids: BTreeSet<_> = after_steps.keys().cloned().collect();
    let added_steps = after_ids
        .difference(&before_ids)
        .filter_map(|id| after_steps.get(id))
        .map(|item| item.descriptor())
        .collect();
    let removed_steps = before_ids
        .difference(&after_ids)
        .filter_map(|id| before_steps.get(id))
        .map(|item| item.descriptor())
        .collect();
    let modified_steps = before_ids
        .intersection(&after_ids)
        .filter_map(|id| {
            let before = before_steps.get(id)?;
            let after = after_steps.get(id)?;
            if before.canonical != after.canonical {
                Some(after.descriptor())
            } else {
                None
            }
        })
        .collect();

    let before_group_names: BTreeSet<_> = before_groups.keys().cloned().collect();
    let after_group_names: BTreeSet<_> = after_groups.keys().cloned().collect();
    let added_groups = after_group_names
        .difference(&before_group_names)
        .cloned()
        .collect();
    let removed_groups = before_group_names
        .difference(&after_group_names)
        .cloned()
        .collect();
    let modified_groups = before_group_names
        .intersection(&after_group_names)
        .filter_map(|name| {
            let before = before_groups.get(name)?;
            let after = after_groups.get(name)?;
            if before != after {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    ReleaseContentDiffSummary {
        baseline_version: baseline_version.map(str::to_string),
        step_count_before: before_steps.len() as i32,
        step_count_after: after_steps.len() as i32,
        group_count_before: before_groups.len() as i32,
        group_count_after: after_groups.len() as i32,
        added_steps,
        removed_steps,
        modified_steps,
        added_groups,
        removed_groups,
        modified_groups,
        input_schema_changed: extract_schema_len(baseline, "inputSchema")
            != extract_schema_len(Some(target), "inputSchema"),
        output_schema_changed: extract_schema_len(baseline, "outputSchema")
            != extract_schema_len(Some(target), "outputSchema"),
        tags_changed: extract_string_array(
            baseline.and_then(|value| value.get("config").and_then(|cfg| cfg.get("tags"))),
        ) != extract_string_array(
            target.get("config").and_then(|cfg| cfg.get("tags")),
        ),
        description_changed: extract_optional_string(
            baseline.and_then(|value| value.get("config").and_then(|cfg| cfg.get("description"))),
        ) != extract_optional_string(
            target.get("config").and_then(|cfg| cfg.get("description")),
        ),
    }
}

#[derive(Clone)]
struct StepSnapshot {
    id: String,
    name: String,
    step_type: Option<String>,
    canonical: String,
}

impl StepSnapshot {
    fn descriptor(&self) -> ReleaseStepDiffItem {
        ReleaseStepDiffItem {
            id: self.id.clone(),
            name: self.name.clone(),
            step_type: self.step_type.clone(),
        }
    }
}

fn extract_steps(snapshot: &JsonValue) -> BTreeMap<String, StepSnapshot> {
    let mut items = BTreeMap::new();
    match snapshot.get("steps") {
        Some(JsonValue::Array(steps)) => {
            for step in steps {
                if let Some((id, item)) = extract_step_snapshot(step) {
                    items.insert(id, item);
                }
            }
        }
        Some(JsonValue::Object(steps)) => {
            for step in steps.values() {
                if let Some((id, item)) = extract_step_snapshot(step) {
                    items.insert(id, item);
                }
            }
        }
        _ => {}
    }
    items
}

fn extract_step_snapshot(step: &JsonValue) -> Option<(String, StepSnapshot)> {
    let id = step.get("id")?.as_str()?.to_string();
    let name = step
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or(&id)
        .to_string();
    let step_type = step
        .get("type")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let canonical = serde_json::to_string(step).ok()?;
    Some((
        id.clone(),
        StepSnapshot {
            id,
            name,
            step_type,
            canonical,
        },
    ))
}

fn extract_groups(snapshot: &JsonValue) -> BTreeMap<String, String> {
    let mut items = BTreeMap::new();
    if let Some(JsonValue::Array(groups)) = snapshot.get("groups") {
        for group in groups {
            let name = group
                .get("name")
                .and_then(|value| value.as_str())
                .or_else(|| group.get("id").and_then(|value| value.as_str()));
            if let Some(name) = name {
                let canonical = serde_json::to_string(group).unwrap_or_default();
                items.insert(name.to_string(), canonical);
            }
        }
    }
    items
}

fn extract_schema_len(snapshot: Option<&JsonValue>, field: &str) -> usize {
    snapshot
        .and_then(|value| value.get("config"))
        .and_then(|config| config.get(field))
        .and_then(|value| value.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn extract_string_array(value: Option<&JsonValue>) -> Vec<String> {
    value
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_optional_string(value: Option<&JsonValue>) -> Option<String> {
    value.and_then(|item| item.as_str()).map(str::to_string)
}

pub(super) fn extract_ruleset_version(snapshot: &JsonValue) -> Option<&str> {
    snapshot
        .get("config")
        .and_then(|config| config.get("version"))
        .and_then(|value| value.as_str())
}
