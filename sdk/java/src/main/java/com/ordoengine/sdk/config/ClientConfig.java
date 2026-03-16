package com.ordoengine.sdk.config;

import java.time.Duration;

public class ClientConfig {
    private final String httpAddress;
    private final String grpcAddress;
    private final boolean preferGrpc;
    private final boolean httpOnly;
    private final boolean grpcOnly;
    private final String tenantId;
    private final Duration timeout;
    private final RetryConfig retryConfig;
    private final int batchConcurrency;

    ClientConfig(Builder builder) {
        this.httpAddress = builder.httpAddress;
        this.grpcAddress = builder.grpcAddress;
        this.preferGrpc = builder.preferGrpc;
        this.httpOnly = builder.httpOnly;
        this.grpcOnly = builder.grpcOnly;
        this.tenantId = builder.tenantId;
        this.timeout = builder.timeout;
        this.retryConfig = builder.retryConfig;
        this.batchConcurrency = builder.batchConcurrency;
    }

    public String getHttpAddress() { return httpAddress; }
    public String getGrpcAddress() { return grpcAddress; }
    public boolean isPreferGrpc() { return preferGrpc; }
    public boolean isHttpOnly() { return httpOnly; }
    public boolean isGrpcOnly() { return grpcOnly; }
    public String getTenantId() { return tenantId; }
    public Duration getTimeout() { return timeout; }
    public RetryConfig getRetryConfig() { return retryConfig; }
    public int getBatchConcurrency() { return batchConcurrency; }

    public static class Builder {
        private String httpAddress = "http://localhost:8080";
        private String grpcAddress;
        private boolean preferGrpc = true;
        private boolean httpOnly = false;
        private boolean grpcOnly = false;
        private String tenantId;
        private Duration timeout = Duration.ofSeconds(30);
        private RetryConfig retryConfig;
        private int batchConcurrency = 10;

        public Builder httpAddress(String httpAddress) {
            this.httpAddress = httpAddress;
            return this;
        }

        public Builder grpcAddress(String grpcAddress) {
            this.grpcAddress = grpcAddress;
            return this;
        }

        public Builder preferGrpc(boolean preferGrpc) {
            this.preferGrpc = preferGrpc;
            return this;
        }

        public Builder httpOnly(boolean httpOnly) {
            this.httpOnly = httpOnly;
            return this;
        }

        public Builder grpcOnly(boolean grpcOnly) {
            this.grpcOnly = grpcOnly;
            return this;
        }

        public Builder tenantId(String tenantId) {
            this.tenantId = tenantId;
            return this;
        }

        public Builder timeout(Duration timeout) {
            this.timeout = timeout;
            return this;
        }

        public Builder retry(RetryConfig retryConfig) {
            this.retryConfig = retryConfig;
            return this;
        }

        public Builder batchConcurrency(int batchConcurrency) {
            this.batchConcurrency = batchConcurrency;
            return this;
        }

        public ClientConfig build() {
            return new ClientConfig(this);
        }
    }
}
