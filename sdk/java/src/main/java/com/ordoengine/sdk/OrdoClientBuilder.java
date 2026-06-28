package com.ordoengine.sdk;

import com.ordoengine.sdk.config.ClientConfig;
import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.exception.ConfigException;

import java.time.Duration;

public class OrdoClientBuilder {
    private String httpAddress = "http://localhost:8080";
    private String grpcAddress;
    private boolean preferGrpc = true;
    private boolean httpOnly = false;
    private boolean grpcOnly = false;
    private String tenantId;
    private Duration timeout = Duration.ofSeconds(30);
    private RetryConfig retryConfig;
    private int batchConcurrency = 10;

    OrdoClientBuilder() {}

    public OrdoClientBuilder httpAddress(String httpAddress) {
        this.httpAddress = httpAddress;
        return this;
    }

    public OrdoClientBuilder grpcAddress(String grpcAddress) {
        this.grpcAddress = grpcAddress;
        return this;
    }

    public OrdoClientBuilder preferGrpc(boolean preferGrpc) {
        this.preferGrpc = preferGrpc;
        return this;
    }

    public OrdoClientBuilder httpOnly(boolean httpOnly) {
        this.httpOnly = httpOnly;
        return this;
    }

    public OrdoClientBuilder grpcOnly(boolean grpcOnly) {
        this.grpcOnly = grpcOnly;
        return this;
    }

    public OrdoClientBuilder tenantId(String tenantId) {
        this.tenantId = tenantId;
        return this;
    }

    public OrdoClientBuilder timeout(Duration timeout) {
        this.timeout = timeout;
        return this;
    }

    public OrdoClientBuilder retry(RetryConfig retryConfig) {
        this.retryConfig = retryConfig;
        return this;
    }

    public OrdoClientBuilder batchConcurrency(int batchConcurrency) {
        this.batchConcurrency = batchConcurrency;
        return this;
    }

    public OrdoClient build() {
        if (httpOnly && grpcOnly) {
            throw new ConfigException("Cannot set both httpOnly and grpcOnly");
        }
        if (grpcOnly && (grpcAddress == null || grpcAddress.isEmpty())) {
            throw new ConfigException("grpcAddress is required when grpcOnly is true");
        }
        return new OrdoClientImpl(
                httpAddress, grpcAddress, preferGrpc, httpOnly, grpcOnly,
                tenantId, timeout, retryConfig, batchConcurrency
        );
    }
}
