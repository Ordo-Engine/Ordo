package types

import "fmt"

// APIError represents an error from the Ordo API
type APIError struct {
	Code       string
	Message    string
	StatusCode int
}

// Error implements the error interface
func (e *APIError) Error() string {
	if e == nil {
		return ""
	}
	if e.StatusCode > 0 {
		return fmt.Sprintf("ordo api error: code=%s status=%d message=%s", e.Code, e.StatusCode, e.Message)
	}
	return fmt.Sprintf("ordo api error: code=%s message=%s", e.Code, e.Message)
}

// ConfigError represents a configuration error
type ConfigError struct {
	Message string
}

// Error implements the error interface
func (e *ConfigError) Error() string {
	if e == nil {
		return ""
	}
	return fmt.Sprintf("ordo config error: %s", e.Message)
}
