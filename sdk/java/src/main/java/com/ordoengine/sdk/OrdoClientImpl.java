package com.ordoengine.sdk;

import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.exception.ConfigException;
import com.ordoengine.sdk.model.*;
import com.ordoengine.sdk.retry.RetryExecutor;
import com.ordoengine.sdk.transport.HttpTransport;

import java.time.Duration;
import java.util.List;

class OrdoClientImpl implements OrdoClient {
    private final HttpTransport httpTransport;
    private final RetryExecutor retryExecutor;
    private final boolean grpcOnly;

    OrdoClientImpl(
            String httpAddress,
            String grpcAddress,
            boolean preferGrpc,
            boolean httpOnly,
            boolean grpcOnly,
            String tenantId,
            Duration timeout,
            RetryConfig retryConfig,
            int batchConcurrency
    ) {
        this.grpcOnly = grpcOnly;

        if (!grpcOnly) {
            this.httpTransport = new HttpTransport(httpAddress, tenantId, timeout);
        } else {
            this.httpTransport = null;
        }

        if (retryConfig != null) {
            this.retryExecutor = new RetryExecutor(retryConfig);
        } else {
            this.retryExecutor = null;
        }

        // gRPC transport is not yet implemented — falls back to HTTP
    }

    private HttpTransport requireHttp() {
        if (httpTransport == null) {
            throw new ConfigException("This operation requires HTTP transport (not available in gRPC-only mode)");
        }
        return httpTransport;
    }

    private <T> T withRetry(java.util.concurrent.Callable<T> fn) {
        if (retryExecutor != null) {
            return retryExecutor.execute(fn);
        }
        try {
            return fn.call();
        } catch (RuntimeException e) {
            throw e;
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    // --- Execution ---

    @Override
    public ExecuteResult execute(String name, Object input) {
        return execute(name, input, false);
    }

    @Override
    public ExecuteResult execute(String name, Object input, boolean includeTrace) {
        return withRetry(() -> requireHttp().execute(name, input, includeTrace));
    }

    @Override
    public BatchResult executeBatch(String name, List<?> inputs) {
        return executeBatch(name, inputs, false);
    }

    @Override
    public BatchResult executeBatch(String name, List<?> inputs, boolean includeTrace) {
        return withRetry(() -> requireHttp().executeBatch(name, inputs, includeTrace));
    }

    // --- Rule Management ---

    @Override
    public List<RuleSetSummary> listRuleSets() {
        return requireHttp().listRuleSets();
    }

    @Override
    public RuleSet getRuleSet(String name) {
        return requireHttp().getRuleSet(name);
    }

    @Override
    public void createRuleSet(Object ruleset) {
        requireHttp().createRuleSet(ruleset);
    }

    @Override
    public void updateRuleSet(String name, Object ruleset) {
        requireHttp().updateRuleSet(name, ruleset);
    }

    @Override
    public void deleteRuleSet(String name) {
        requireHttp().deleteRuleSet(name);
    }

    // --- Version Management ---

    @Override
    public VersionList listVersions(String name) {
        return requireHttp().listVersions(name);
    }

    @Override
    public RollbackResult rollback(String name, int seq) {
        return requireHttp().rollback(name, seq);
    }

    // --- Eval ---

    @Override
    public EvalResult eval(String expression, Object context) {
        return withRetry(() -> requireHttp().eval(expression, context));
    }

    // --- Health ---

    @Override
    public HealthStatus health() {
        return withRetry(() -> requireHttp().health());
    }

    // --- Lifecycle ---

    @Override
    public void close() {
        if (httpTransport != null) {
            httpTransport.close();
        }
    }
}
