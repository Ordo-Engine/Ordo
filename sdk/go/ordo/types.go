package ordo

import (
	"github.com/pama-lee/ordo-go/ordo/degrade"
	"github.com/pama-lee/ordo-go/ordo/types"
)

// Re-export types for convenience
type (
	RuleSetConfig   = types.RuleSetConfig
	RuleSet         = types.RuleSet
	RuleSetSummary  = types.RuleSetSummary
	ExecuteResult   = types.ExecuteResult
	ExecutionTrace  = types.ExecutionTrace
	StepTrace       = types.StepTrace
	BatchResult     = types.BatchResult
	BatchItem       = types.BatchItem
	BatchSummary    = types.BatchSummary
	EvalResult      = types.EvalResult
	VersionList     = types.VersionList
	VersionInfo     = types.VersionInfo
	RollbackResult  = types.RollbackResult
	HealthStatus    = types.HealthStatus
	StorageStatus   = types.StorageStatus
	APIError        = types.APIError
	ConfigError     = types.ConfigError
	DegradationMode = degrade.Mode
	DegradeConfig   = degrade.Config
)

// Degradation mode constants, re-exported for convenience.
const (
	DegradeFail     = degrade.ModeFail
	DegradeStale    = degrade.ModeStale
	DegradeFailOpen = degrade.ModeFailOpen
)
