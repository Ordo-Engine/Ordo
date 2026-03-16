import com.ordoengine.sdk.OrdoClient;
import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.model.ExecuteResult;

import java.time.Duration;
import java.util.Map;

public class RetryExample {
    public static void main(String[] args) {
        // Client with retry and multi-tenancy
        OrdoClient client = OrdoClient.builder()
                .httpAddress("http://localhost:8080")
                .tenantId("tenant-abc")
                .retry(RetryConfig.builder()
                        .maxAttempts(3)
                        .initialInterval(Duration.ofMillis(100))
                        .maxInterval(Duration.ofSeconds(5))
                        .jitter(true)
                        .build())
                .build();

        // Requests automatically retry on 5xx / 429 / connection errors
        ExecuteResult result = client.execute("risk_check", Map.of(
                "user_id", "u123",
                "action", "transfer"
        ));
        System.out.printf("Result: %s - %s%n", result.getCode(), result.getMessage());

        client.close();
    }
}
