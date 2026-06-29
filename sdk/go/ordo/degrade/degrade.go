// Package degrade adds opt-in offline resilience to the Ordo client: a
// last-known-good snapshot cache plus a configurable degradation policy that is
// applied only after retries are exhausted. With no degradation configured the
// client behaves exactly as before — every failure is returned to the caller.
package degrade

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"time"

	"github.com/pama-lee/ordo-go/ordo/types"
)

// Mode selects what happens when the engine is unreachable after retries.
type Mode int

const (
	// ModeFail propagates the error (default; preserves current behavior).
	ModeFail Mode = iota
	// ModeStale serves the last-known-good cached result for the same
	// ruleset+input, flagged stale.
	ModeStale
	// ModeFailOpen returns a caller-supplied fallback result, flagged stale.
	ModeFailOpen
)

// Config configures snapshot caching and the degradation policy.
type Config struct {
	// Mode is the policy applied on failure after retries.
	Mode Mode
	// TTL is the lifetime of a cached snapshot (default 5m).
	TTL time.Duration
	// MaxEntries bounds the cache size with LRU eviction (default 1024).
	MaxEntries int
	// DiskPath, when set, persists snapshots so they survive restarts.
	DiskPath string
	// Fallback is returned by Execute under ModeFailOpen.
	Fallback *types.ExecuteResult
	// EvalFallback is returned by Eval under ModeFailOpen.
	EvalFallback *types.EvalResult
}

// Degrader couples the snapshot cache with the configured policy.
type Degrader struct {
	cfg   Config
	cache *Cache
}

// New builds a Degrader from cfg.
func New(cfg Config) *Degrader {
	return &Degrader{
		cfg:   cfg,
		cache: NewCache(cfg.TTL, cfg.MaxEntries, cfg.DiskPath),
	}
}

// StoreExecute caches a successful execute result.
func (d *Degrader) StoreExecute(name string, input any, result *types.ExecuteResult) {
	if d == nil || result == nil {
		return
	}
	d.cache.Put(execKey(name, input), result)
}

// StoreEval caches a successful eval result.
func (d *Degrader) StoreEval(expr string, context any, result *types.EvalResult) {
	if d == nil || result == nil {
		return
	}
	d.cache.Put(evalKey(expr, context), result)
}

// ExecuteFallback returns a degraded execute result for a failed call. The
// boolean is false when the policy cannot serve anything, in which case the
// caller should return the original error.
func (d *Degrader) ExecuteFallback(name string, input any) (*types.ExecuteResult, bool) {
	if d == nil {
		return nil, false
	}
	switch d.cfg.Mode {
	case ModeStale:
		var r types.ExecuteResult
		if d.cache.Get(execKey(name, input), &r) {
			r.Stale = true
			return &r, true
		}
	case ModeFailOpen:
		if d.cfg.Fallback != nil {
			r := *d.cfg.Fallback
			r.Stale = true
			return &r, true
		}
	}
	return nil, false
}

// EvalFallback returns a degraded eval result for a failed call.
func (d *Degrader) EvalFallback(expr string, context any) (*types.EvalResult, bool) {
	if d == nil {
		return nil, false
	}
	switch d.cfg.Mode {
	case ModeStale:
		var r types.EvalResult
		if d.cache.Get(evalKey(expr, context), &r) {
			r.Stale = true
			return &r, true
		}
	case ModeFailOpen:
		if d.cfg.EvalFallback != nil {
			r := *d.cfg.EvalFallback
			r.Stale = true
			return &r, true
		}
	}
	return nil, false
}

func execKey(name string, input any) string {
	return hashKey("execute", name, input)
}

func evalKey(expr string, context any) string {
	return hashKey("eval", expr, context)
}

func hashKey(op, name string, payload any) string {
	h := sha256.New()
	h.Write([]byte(op))
	h.Write([]byte{0})
	h.Write([]byte(name))
	h.Write([]byte{0})
	if b, err := json.Marshal(payload); err == nil {
		h.Write(b)
	}
	return hex.EncodeToString(h.Sum(nil))
}
