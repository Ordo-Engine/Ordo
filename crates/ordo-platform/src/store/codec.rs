use super::*;

pub(super) fn role_to_str(r: &Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
    }
}

pub(super) fn str_to_role(s: &str) -> Result<Role> {
    s.parse().map_err(|e: String| anyhow::anyhow!(e))
}

pub(super) fn fact_data_type_to_str(t: &FactDataType) -> &'static str {
    match t {
        FactDataType::String => "string",
        FactDataType::Number => "number",
        FactDataType::Boolean => "boolean",
        FactDataType::Date => "date",
        FactDataType::Object => "object",
    }
}

pub(super) fn str_to_fact_data_type(s: &str) -> Result<FactDataType> {
    match s {
        "string" => Ok(FactDataType::String),
        "number" => Ok(FactDataType::Number),
        "boolean" => Ok(FactDataType::Boolean),
        "date" => Ok(FactDataType::Date),
        "object" => Ok(FactDataType::Object),
        other => Err(anyhow::anyhow!("invalid data type: {}", other)),
    }
}

pub(super) fn null_policy_to_str(p: &NullPolicy) -> &'static str {
    match p {
        NullPolicy::Error => "error",
        NullPolicy::Default => "default",
        NullPolicy::Skip => "skip",
    }
}

pub(super) fn str_to_null_policy(s: &str) -> Result<NullPolicy> {
    match s {
        "error" => Ok(NullPolicy::Error),
        "default" => Ok(NullPolicy::Default),
        "skip" => Ok(NullPolicy::Skip),
        other => Err(anyhow::anyhow!("invalid null policy: {}", other)),
    }
}

pub(super) fn history_source_to_str(s: &RulesetHistorySource) -> &'static str {
    match s {
        RulesetHistorySource::Sync => "sync",
        RulesetHistorySource::Edit => "edit",
        RulesetHistorySource::Save => "save",
        RulesetHistorySource::Restore => "restore",
        RulesetHistorySource::Create => "create",
        RulesetHistorySource::Publish => "publish",
    }
}

pub(super) fn str_to_history_source(s: &str) -> Result<RulesetHistorySource> {
    match s {
        "sync" => Ok(RulesetHistorySource::Sync),
        "edit" => Ok(RulesetHistorySource::Edit),
        "save" => Ok(RulesetHistorySource::Save),
        "restore" => Ok(RulesetHistorySource::Restore),
        "create" => Ok(RulesetHistorySource::Create),
        "publish" => Ok(RulesetHistorySource::Publish),
        other => Err(anyhow::anyhow!("invalid history source: {}", other)),
    }
}
