# Ordo Java SDK

Java SDK for the [Ordo Rule Engine](https://github.com/Pama-Lee/Ordo) — a high-performance rule engine with sub-microsecond execution latency.

## Requirements

- Java 11+
- Maven 3.6+

## Installation

### Maven

```xml
<dependency>
    <groupId>com.ordoengine</groupId>
    <artifactId>ordo-sdk-java</artifactId>
    <version>0.3.0</version>
</dependency>
```

### Gradle

```groovy
implementation 'com.ordoengine:ordo-sdk-java:0.3.0'
```

## Quick Start

```java
import com.ordoengine.sdk.OrdoClient;
import com.ordoengine.sdk.model.ExecuteResult;
import java.util.Map;

OrdoClient client = OrdoClient.builder()
    .httpAddress("http://localhost:8080")
    .build();

ExecuteResult result = client.execute("payment_check", Map.of(
    "amount", 1500,
    "currency", "USD"
));
System.out.printf("%s: %s%n", result.getCode(), result.getMessage());

client.close();
```

## Client Configuration

```java
import com.ordoengine.sdk.OrdoClient;
import com.ordoengine.sdk.config.RetryConfig;
import java.time.Duration;

OrdoClient client = OrdoClient.builder()
    .httpAddress("http://localhost:8080")   // HTTP endpoint
    .tenantId("my-tenant")                 // Multi-tenancy
    .timeout(Duration.ofSeconds(30))       // HTTP timeout
    .retry(RetryConfig.builder()           // Retry with backoff
        .maxAttempts(3)
        .initialInterval(Duration.ofMillis(100))
        .maxInterval(Duration.ofSeconds(5))
        .jitter(true)
        .build())
    .batchConcurrency(10)                  // Parallel batch limit
    .build();
```

## API Reference

### Execution

```java
// Single execution
ExecuteResult result = client.execute("ruleset_name", inputMap);
ExecuteResult result = client.execute("ruleset_name", inputMap, true); // with trace

// Batch execution
BatchResult batch = client.executeBatch("ruleset_name", List.of(input1, input2));
```

### Rule Management (HTTP only)

```java
List<RuleSetSummary> rulesets = client.listRuleSets();
RuleSet ruleset = client.getRuleSet("name");
client.createRuleSet(rulesetMap);
client.updateRuleSet("name", rulesetMap);
client.deleteRuleSet("name");
```

### Version Management

```java
VersionList versions = client.listVersions("ruleset_name");
RollbackResult rollback = client.rollback("ruleset_name", 2);
```

### Expression Evaluation

```java
EvalResult eval = client.eval("age > 18 && status == 'active'", Map.of("age", 25, "status", "active"));
```

### Health Check

```java
HealthStatus health = client.health();
System.out.printf("%s, %d rulesets%n", health.getStatus(), health.getRulesetCount());
```

## Error Handling

```java
import com.ordoengine.sdk.exception.*;

try {
    ExecuteResult result = client.execute("my_rule", data);
} catch (ApiException e) {
    System.out.printf("API error: %s (code=%s, status=%d)%n",
        e.getMessage(), e.getErrorCode(), e.getStatusCode());
} catch (ConnectionException e) {
    System.out.printf("Connection failed: %s%n", e.getMessage());
}
```

### Retry

Retryable errors (automatically retried when `retry` is configured):
- HTTP 5xx server errors
- HTTP 429 rate limiting
- Connection errors and timeouts

Non-retryable errors (raised immediately):
- HTTP 4xx client errors (except 429)

## Data Models

| Class | Key Fields |
|-------|------------|
| `ExecuteResult` | `code`, `message`, `output`, `durationUs`, `trace` |
| `BatchResult` | `results: List<ExecuteResultItem>`, `summary: BatchSummary` |
| `RuleSetSummary` | `name`, `version`, `description`, `stepCount` |
| `VersionList` | `name`, `currentVersion`, `versions: List<VersionInfo>` |
| `EvalResult` | `result`, `parsed` |
| `HealthStatus` | `status`, `version`, `rulesetCount`, `uptimeSeconds`, `storage` |

## Building from Source

```bash
cd sdk/java
mvn clean compile
mvn test
mvn package
```

## License

MIT
