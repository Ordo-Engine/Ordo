package com.ordoengine.sdk.config;

import java.time.Duration;

public class RetryConfig {
    private final int maxAttempts;
    private final Duration initialInterval;
    private final Duration maxInterval;
    private final boolean jitter;

    private RetryConfig(Builder builder) {
        this.maxAttempts = builder.maxAttempts;
        this.initialInterval = builder.initialInterval;
        this.maxInterval = builder.maxInterval;
        this.jitter = builder.jitter;
    }

    public static Builder builder() {
        return new Builder();
    }

    public static RetryConfig defaults() {
        return builder().build();
    }

    public int getMaxAttempts() { return maxAttempts; }
    public Duration getInitialInterval() { return initialInterval; }
    public Duration getMaxInterval() { return maxInterval; }
    public boolean isJitter() { return jitter; }

    public static class Builder {
        private int maxAttempts = 3;
        private Duration initialInterval = Duration.ofMillis(100);
        private Duration maxInterval = Duration.ofSeconds(5);
        private boolean jitter = true;

        public Builder maxAttempts(int maxAttempts) {
            this.maxAttempts = maxAttempts;
            return this;
        }

        public Builder initialInterval(Duration initialInterval) {
            this.initialInterval = initialInterval;
            return this;
        }

        public Builder maxInterval(Duration maxInterval) {
            this.maxInterval = maxInterval;
            return this;
        }

        public Builder jitter(boolean jitter) {
            this.jitter = jitter;
            return this;
        }

        public RetryConfig build() {
            return new RetryConfig(this);
        }
    }
}
