package types

import "encoding/json"

// RuleSetConfig defines the configuration of a ruleset
type RuleSetConfig struct {
	Name      string `json:"name"`
	Version   string `json:"version,omitempty"`
	EntryStep string `json:"entry_step"`
}

// RuleSet defines a complete ruleset
type RuleSet struct {
	Config RuleSetConfig              `json:"config"`
	Steps  map[string]json.RawMessage `json:"steps"`
}

// RuleSetSummary is a brief summary of a ruleset
type RuleSetSummary struct {
	Name        string  `json:"name"`
	Version     string  `json:"version"`
	Description *string `json:"description"`
	StepCount   uint32  `json:"step_count,omitempty"`
}

// ExecuteResult is the result of a rule execution
type ExecuteResult struct {
	Code       string          `json:"code"`
	Message    string          `json:"message"`
	Output     json.RawMessage `json:"output"`
	DurationUs uint64          `json:"duration_us"`
	Trace      *ExecutionTrace `json:"trace,omitempty"`
}

// ExecutionTrace contains execution trace information
type ExecutionTrace struct {
	Path  string      `json:"path"`
	Steps []StepTrace `json:"steps"`
}

// StepTrace contains trace information for a single step
type StepTrace struct {
	StepID     string `json:"step_id"`
	StepName   string `json:"step_name"`
	DurationUs uint64 `json:"duration_us"`
	Result     string `json:"result,omitempty"`
}

// BatchResult is the result of a batch execution
type BatchResult struct {
	Results []ExecuteResultItem `json:"results"`
	Summary BatchSummary        `json:"summary"`
}

// ExecuteResultItem is a single item in batch results (used by both HTTP and gRPC)
type ExecuteResultItem struct {
	Code       string          `json:"code"`
	Message    string          `json:"message"`
	Output     json.RawMessage `json:"output"`
	DurationUs uint64          `json:"duration_us"`
	Trace      *ExecutionTrace `json:"trace,omitempty"`
	Error      *string         `json:"error,omitempty"`
}

// BatchItem is an alias for ExecuteResultItem (for backward compatibility)
type BatchItem = ExecuteResultItem

// BatchSummary is the summary of a batch execution
type BatchSummary struct {
	Total           uint32 `json:"total"`
	Success         uint32 `json:"success"`
	Failed          uint32 `json:"failed"`
	TotalDurationUs uint64 `json:"total_duration_us"`
}

// EvalResult is the result of expression evaluation
type EvalResult struct {
	Result json.RawMessage `json:"result"`
	Parsed string          `json:"parsed"`
}

// VersionList contains version information for a ruleset
type VersionList struct {
	Name           string        `json:"name"`
	CurrentVersion string        `json:"current_version"`
	Versions       []VersionInfo `json:"versions"`
}

// VersionInfo contains information about a single version
type VersionInfo struct {
	Seq       int    `json:"seq"`
	Version   string `json:"version"`
	Timestamp string `json:"timestamp"`
}

// RollbackResult is the result of a rollback operation
type RollbackResult struct {
	Status      string `json:"status"`
	Name        string `json:"name"`
	FromVersion string `json:"from_version"`
	ToVersion   string `json:"to_version"`
}

// HealthStatus contains server health information
type HealthStatus struct {
	Status        string         `json:"status,omitempty"`
	Version       string         `json:"version,omitempty"`
	RulesetCount  uint32         `json:"ruleset_count,omitempty"`
	UptimeSeconds uint64         `json:"uptime_seconds,omitempty"`
	Storage       *StorageStatus `json:"storage,omitempty"`
}

// StorageStatus contains storage status information
type StorageStatus struct {
	Mode       string `json:"mode"`
	RulesDir   string `json:"rules_dir,omitempty"`
	RulesCount uint32 `json:"rules_count,omitempty"`
}
