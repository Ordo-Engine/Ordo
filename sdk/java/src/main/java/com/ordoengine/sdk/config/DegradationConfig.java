package com.ordoengine.sdk.config;

import com.ordoengine.sdk.model.EvalResult;
import com.ordoengine.sdk.model.ExecuteResult;

import java.time.Duration;

/**
 * Configuration for the local-snapshot cache and the degradation policy applied
 * when the engine is unreachable after retries are exhausted.
 *
 * <p>Degradation is strictly opt-in. When no {@code DegradationConfig} is set on
 * the client, every failure is thrown to the caller (the default behavior).
 */
public class DegradationConfig {
    private final DegradationMode mode;
    private final Duration ttl;
    private final int maxEntries;
    private final String diskPath;
    private final ExecuteResult fallback;
    private final EvalResult evalFallback;

    private DegradationConfig(Builder builder) {
        this.mode = builder.mode;
        this.ttl = builder.ttl;
        this.maxEntries = builder.maxEntries;
        this.diskPath = builder.diskPath;
        this.fallback = builder.fallback;
        this.evalFallback = builder.evalFallback;
    }

    public static Builder builder() {
        return new Builder();
    }

    public DegradationMode getMode() { return mode; }
    public Duration getTtl() { return ttl; }
    public int getMaxEntries() { return maxEntries; }
    public String getDiskPath() { return diskPath; }
    public ExecuteResult getFallback() { return fallback; }
    public EvalResult getEvalFallback() { return evalFallback; }

    public static class Builder {
        private DegradationMode mode = DegradationMode.FAIL;
        private Duration ttl = Duration.ofMinutes(5);
        private int maxEntries = 1024;
        private String diskPath;
        private ExecuteResult fallback;
        private EvalResult evalFallback;

        /** Policy applied on failure after retries. */
        public Builder mode(DegradationMode mode) {
            this.mode = mode;
            return this;
        }

        /** Lifetime of a cached snapshot. */
        public Builder ttl(Duration ttl) {
            this.ttl = ttl;
            return this;
        }

        /** Maximum number of cached snapshots (LRU eviction). */
        public Builder maxEntries(int maxEntries) {
            this.maxEntries = maxEntries;
            return this;
        }

        /** Optional file path to persist the cache across restarts. */
        public Builder diskPath(String diskPath) {
            this.diskPath = diskPath;
            return this;
        }

        /** Result returned by {@code execute} under {@link DegradationMode#FAIL_OPEN}. */
        public Builder fallback(ExecuteResult fallback) {
            this.fallback = fallback;
            return this;
        }

        /** Result returned by {@code eval} under {@link DegradationMode#FAIL_OPEN}. */
        public Builder evalFallback(EvalResult evalFallback) {
            this.evalFallback = evalFallback;
            return this;
        }

        public DegradationConfig build() {
            return new DegradationConfig(this);
        }
    }
}
