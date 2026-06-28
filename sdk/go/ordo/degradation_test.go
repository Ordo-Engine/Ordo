package ordo

import (
	"context"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"

	"github.com/pama-lee/ordo-go/ordo/degrade"
)

// newSwitchableServer returns a test server that serves a successful execute
// response until fail is set, after which it returns 503 (engine unreachable).
func newSwitchableServer(t *testing.T) (*httptest.Server, *atomic.Bool) {
	t.Helper()
	var fail atomic.Bool
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if fail.Load() {
			w.WriteHeader(http.StatusServiceUnavailable)
			_, _ = w.Write([]byte(`{"error":"engine down"}`))
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"code":"APPROVE","message":"ok","output":{"score":1},"duration_us":5}`))
	}))
	t.Cleanup(srv.Close)
	return srv, &fail
}

func TestExecuteStaleServesCachedResult(t *testing.T) {
	srv, fail := newSwitchableServer(t)
	c, err := NewClient(
		WithHTTPAddress(srv.URL),
		WithHTTPOnly(),
		WithDegradation(degrade.Config{Mode: degrade.ModeStale}),
	)
	if err != nil {
		t.Fatalf("NewClient: %v", err)
	}
	defer c.Close()

	ctx := context.Background()
	input := map[string]any{"amount": 100}

	// Prime the snapshot cache with a fresh, successful result.
	first, err := c.Execute(ctx, "loan", input)
	if err != nil {
		t.Fatalf("priming Execute: %v", err)
	}
	if first.Code != "APPROVE" || first.Stale {
		t.Fatalf("fresh result wrong: code=%q stale=%v", first.Code, first.Stale)
	}

	// Engine goes down: the cached last-known-good is served, flagged stale.
	fail.Store(true)
	degraded, err := c.Execute(ctx, "loan", input)
	if err != nil {
		t.Fatalf("expected stale result, got error: %v", err)
	}
	if degraded.Code != "APPROVE" || !degraded.Stale {
		t.Fatalf("expected stale APPROVE, got code=%q stale=%v", degraded.Code, degraded.Stale)
	}
}

func TestExecuteDefaultConfigFailsHard(t *testing.T) {
	srv, fail := newSwitchableServer(t)
	c, err := NewClient(WithHTTPAddress(srv.URL), WithHTTPOnly())
	if err != nil {
		t.Fatalf("NewClient: %v", err)
	}
	defer c.Close()

	fail.Store(true)
	if _, err := c.Execute(context.Background(), "loan", map[string]any{"amount": 100}); err == nil {
		t.Fatal("default config must propagate the engine error (no degradation)")
	}
}

func TestExecuteStaleWithoutPrimedCacheFails(t *testing.T) {
	srv, fail := newSwitchableServer(t)
	c, err := NewClient(
		WithHTTPAddress(srv.URL),
		WithHTTPOnly(),
		WithDegradation(degrade.Config{Mode: degrade.ModeStale}),
	)
	if err != nil {
		t.Fatalf("NewClient: %v", err)
	}
	defer c.Close()

	fail.Store(true)
	if _, err := c.Execute(context.Background(), "loan", map[string]any{"amount": 100}); err == nil {
		t.Fatal("STALE with an empty cache must still fail")
	}
}
