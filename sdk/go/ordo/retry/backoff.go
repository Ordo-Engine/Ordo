package retry

import (
	"math"
	"math/rand"
	"time"
)

// Backoff defines the interface for backoff strategies
type Backoff interface {
	Next() time.Duration
	Reset()
}

// ExponentialBackoff implements exponential backoff with jitter
type ExponentialBackoff struct {
	InitialInterval time.Duration
	MaxInterval     time.Duration
	Multiplier      float64
	Jitter          bool
	attempt         int
}

// NewExponentialBackoff creates a new exponential backoff
func NewExponentialBackoff(initial, max time.Duration, jitter bool) *ExponentialBackoff {
	return &ExponentialBackoff{
		InitialInterval: initial,
		MaxInterval:     max,
		Multiplier:      2.0,
		Jitter:          jitter,
		attempt:         0,
	}
}

// Next calculates the next backoff duration
func (b *ExponentialBackoff) Next() time.Duration {
	if b.attempt == 0 {
		b.attempt++
		return b.InitialInterval
	}

	// Calculate exponential backoff
	duration := float64(b.InitialInterval) * math.Pow(b.Multiplier, float64(b.attempt))
	
	// Cap at max interval
	if duration > float64(b.MaxInterval) {
		duration = float64(b.MaxInterval)
	}

	b.attempt++

	// Add jitter if enabled
	if b.Jitter {
		jitter := rand.Float64() * 0.3 * duration // Up to 30% jitter
		duration = duration - jitter/2 + rand.Float64()*jitter
	}

	return time.Duration(duration)
}

// Reset resets the backoff state
func (b *ExponentialBackoff) Reset() {
	b.attempt = 0
}
