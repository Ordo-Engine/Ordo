package com.ordoengine.sdk;

import com.ordoengine.sdk.config.ClientConfig;
import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.model.*;

import java.time.Duration;
import java.util.List;
import java.util.Map;

/**
 * Unified client for the Ordo Rule Engine.
 *
 * <p>Supports both HTTP and gRPC transports with automatic protocol selection.
 * Use {@link #builder()} to create instances.
 *
 * <pre>{@code
 * OrdoClient client = OrdoClient.builder()
 *     .httpAddress("http://localhost:8080")
 *     .build();
 *
 * ExecuteResult result = client.execute("my_rule", Map.of("age", 25));
 * System.out.println(result.getCode());
 *
 * client.close();
 * }</pre>
 */
public interface OrdoClient extends AutoCloseable {

    // --- Execution ---

    ExecuteResult execute(String name, Object input);

    ExecuteResult execute(String name, Object input, boolean includeTrace);

    BatchResult executeBatch(String name, List<?> inputs);

    BatchResult executeBatch(String name, List<?> inputs, boolean includeTrace);

    // --- Rule Management (HTTP only) ---

    List<RuleSetSummary> listRuleSets();

    RuleSet getRuleSet(String name);

    void createRuleSet(Object ruleset);

    void updateRuleSet(String name, Object ruleset);

    void deleteRuleSet(String name);

    // --- Version Management (HTTP only) ---

    VersionList listVersions(String name);

    RollbackResult rollback(String name, int seq);

    // --- Eval ---

    EvalResult eval(String expression, Object context);

    // --- Health ---

    HealthStatus health();

    // --- Lifecycle ---

    @Override
    void close();

    // --- Builder ---

    static OrdoClientBuilder builder() {
        return new OrdoClientBuilder();
    }
}
