package degrade

import (
	"encoding/json"
	"path/filepath"
	"testing"
	"time"

	"github.com/pama-lee/ordo-go/ordo/types"
)

func execResult(code string) *types.ExecuteResult {
	return &types.ExecuteResult{Code: code, Message: "ok", Output: json.RawMessage(`{"x":1}`)}
}

func TestStaleServesCachedFlaggedStale(t *testing.T) {
	d := New(Config{Mode: ModeStale})
	d.StoreExecute("loan", map[string]any{"amount": 100}, execResult("APPROVE"))

	got, ok := d.ExecuteFallback("loan", map[string]any{"amount": 100})
	if !ok {
		t.Fatal("expected a cached result to be served")
	}
	if got.Code != "APPROVE" {
		t.Fatalf("expected APPROVE, got %q", got.Code)
	}
	if !got.Stale {
		t.Fatal("served result must be flagged stale")
	}
}

func TestFailModeServesNothing(t *testing.T) {
	d := New(Config{Mode: ModeFail})
	d.StoreExecute("loan", 1, execResult("APPROVE"))
	if _, ok := d.ExecuteFallback("loan", 1); ok {
		t.Fatal("ModeFail must never serve a fallback")
	}
}

func TestNilDegraderIsSafe(t *testing.T) {
	var d *Degrader
	d.StoreExecute("loan", 1, execResult("APPROVE")) // must not panic
	if _, ok := d.ExecuteFallback("loan", 1); ok {
		t.Fatal("nil degrader must not serve a fallback")
	}
}

func TestStaleMissesOnDifferentInput(t *testing.T) {
	d := New(Config{Mode: ModeStale})
	d.StoreExecute("loan", map[string]any{"amount": 100}, execResult("APPROVE"))
	if _, ok := d.ExecuteFallback("loan", map[string]any{"amount": 999}); ok {
		t.Fatal("different input must not hit the cache")
	}
}

func TestFailOpenReturnsFallback(t *testing.T) {
	fb := &types.ExecuteResult{Code: "DEFAULT_DENY", Message: "fallback"}
	d := New(Config{Mode: ModeFailOpen, Fallback: fb})
	got, ok := d.ExecuteFallback("loan", 1)
	if !ok || got.Code != "DEFAULT_DENY" || !got.Stale {
		t.Fatalf("expected stale fallback DEFAULT_DENY, got %+v ok=%v", got, ok)
	}
}

func TestFailOpenWithoutFallbackServesNothing(t *testing.T) {
	d := New(Config{Mode: ModeFailOpen})
	if _, ok := d.ExecuteFallback("loan", 1); ok {
		t.Fatal("FAIL_OPEN without a configured fallback must not serve")
	}
}

func TestEvalDegradation(t *testing.T) {
	d := New(Config{Mode: ModeStale})
	d.StoreEval("1+41", nil, &types.EvalResult{Result: json.RawMessage(`42`), Parsed: "1 + 41"})
	got, ok := d.EvalFallback("1+41", nil)
	if !ok || string(got.Result) != "42" || !got.Stale {
		t.Fatalf("expected stale eval 42, got %+v ok=%v", got, ok)
	}
}

func TestTTLExpiry(t *testing.T) {
	c := NewCache(20*time.Millisecond, 10, "")
	c.Put("k", execResult("APPROVE"))
	var out types.ExecuteResult
	if !c.Get("k", &out) {
		t.Fatal("expected a fresh hit")
	}
	time.Sleep(40 * time.Millisecond)
	if c.Get("k", &out) {
		t.Fatal("entry should have expired")
	}
}

func TestLRUEviction(t *testing.T) {
	c := NewCache(time.Minute, 2, "")
	c.Put("a", execResult("A"))
	c.Put("b", execResult("B"))
	// Touch "a" so "b" becomes least-recently-used.
	var out types.ExecuteResult
	c.Get("a", &out)
	c.Put("c", execResult("C")) // exceeds capacity -> evict "b"
	if c.Get("b", &out) {
		t.Fatal("expected b to be evicted")
	}
	if !c.Get("a", &out) || !c.Get("c", &out) {
		t.Fatal("expected a and c to remain")
	}
}

func TestDiskPersistence(t *testing.T) {
	path := filepath.Join(t.TempDir(), "snapshot.json")

	d1 := New(Config{Mode: ModeStale, DiskPath: path})
	d1.StoreExecute("loan", map[string]any{"amount": 100}, execResult("APPROVE"))

	// New degrader reading the same on-disk cache (simulates a restart).
	d2 := New(Config{Mode: ModeStale, DiskPath: path})
	got, ok := d2.ExecuteFallback("loan", map[string]any{"amount": 100})
	if !ok || got.Code != "APPROVE" || !got.Stale {
		t.Fatalf("expected persisted stale APPROVE, got %+v ok=%v", got, ok)
	}
}
