use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RulesetHistorySource {
    Sync,
    Edit,
    Save,
    Restore,
    Create,
    Publish,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetHistoryEntry {
    pub id: String,
    pub ruleset_name: String,
    pub action: String,
    pub source: RulesetHistorySource,
    pub created_at: DateTime<Utc>,
    pub author_id: String,
    pub author_email: String,
    pub author_display_name: String,
    pub snapshot: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FactDataType {
    String,
    Number,
    Boolean,
    Date,
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NullPolicy {
    Error,
    Default,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactDefinition {
    pub name: String,
    pub data_type: FactDataType,
    pub source: String,
    pub latency_ms: Option<u32>,
    pub null_policy: NullPolicy,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptDefinition {
    pub name: String,
    pub data_type: FactDataType,
    pub expression: String,
    pub dependencies: Vec<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractField {
    pub name: String,
    pub data_type: FactDataType,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContract {
    pub ruleset_name: String,
    pub version_pattern: String,
    pub owner: String,
    pub sla_p99_ms: Option<u32>,
    pub input_fields: Vec<ContractField>,
    pub output_fields: Vec<ContractField>,
    pub notes: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub difficulty: String,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSample {
    pub label: String,
    pub input: JsonValue,
    #[serde(default)]
    pub expected_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDetail {
    #[serde(flatten)]
    pub metadata: TemplateMetadata,
    pub facts: Vec<FactDefinition>,
    pub concepts: Vec<ConceptDefinition>,
    pub ruleset: JsonValue,
    pub samples: Vec<TemplateSample>,
    #[serde(default)]
    pub contract: Option<DecisionContract>,
    #[serde(default)]
    pub tests: Vec<TestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExpectation {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub output: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub input: JsonValue,
    pub expect: TestExpectation,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub test_id: String,
    pub test_name: String,
    pub passed: bool,
    #[serde(default)]
    pub failures: Vec<String>,
    #[serde(default)]
    pub failure_details: Vec<TestFailureDetail>,
    pub duration_us: u64,
    #[serde(default)]
    pub actual_code: Option<String>,
    #[serde(default)]
    pub actual_message: Option<String>,
    #[serde(default)]
    pub actual_output: Option<JsonValue>,
    #[serde(default)]
    pub trace: Option<TestExecutionTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailureDetail {
    pub message: String,
    pub kind: TestFailureKind,
    #[serde(default)]
    pub step_id: Option<String>,
    #[serde(default)]
    pub sub_rule_ref: Option<String>,
    #[serde(default)]
    pub trace_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestFailureKind {
    Reference,
    Contract,
    Binding,
    SubRule,
    Output,
    Assertion,
    Execution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionTrace {
    pub trace_id: String,
    pub path: Vec<String>,
    pub path_string: String,
    pub result_code: String,
    pub total_duration_us: u64,
    #[serde(default)]
    pub error: Option<String>,
    pub steps: Vec<TestExecutionTraceStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionTraceStep {
    pub id: String,
    pub name: String,
    pub duration_us: u64,
    #[serde(default)]
    pub next_step: Option<String>,
    #[serde(default)]
    pub is_terminal: bool,
    #[serde(default)]
    pub input_snapshot: Option<JsonValue>,
    #[serde(default)]
    pub variables_snapshot: Option<JsonValue>,
    #[serde(default)]
    pub sub_rule_ref: Option<String>,
    #[serde(default)]
    pub sub_rule_input: Option<JsonValue>,
    #[serde(default)]
    pub sub_rule_outputs: Vec<TestSubRuleOutputTrace>,
    #[serde(default)]
    pub sub_rule_frames: Vec<TestExecutionTraceStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSubRuleOutputTrace {
    pub parent_var: String,
    pub child_var: String,
    #[serde(default)]
    pub value: Option<JsonValue>,
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTestRunResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub rulesets: Vec<RulesetTestSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetTestSummary {
    pub ruleset_name: String,
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub results: Vec<TestRunResult>,
}
