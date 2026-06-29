package com.ordoengine.sdk.config;

/**
 * Behavior when the Ordo engine is unreachable after retries are exhausted.
 */
public enum DegradationMode {
    /** Propagate the error (default; preserves current behavior). */
    FAIL,
    /** Serve the last-known-good cached result for this ruleset+input, flagged stale. */
    STALE,
    /** Return a caller-supplied fallback result, flagged stale. */
    FAIL_OPEN
}
