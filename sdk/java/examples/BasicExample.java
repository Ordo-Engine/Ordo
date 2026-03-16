import com.ordoengine.sdk.OrdoClient;
import com.ordoengine.sdk.model.ExecuteResult;
import com.ordoengine.sdk.model.HealthStatus;
import com.ordoengine.sdk.model.RuleSetSummary;

import java.util.List;
import java.util.Map;

public class BasicExample {
    public static void main(String[] args) {
        OrdoClient client = OrdoClient.builder()
                .httpAddress("http://localhost:8080")
                .build();

        // Execute a ruleset
        ExecuteResult result = client.execute("payment_check", Map.of(
                "amount", 1500,
                "currency", "USD",
                "user_level", "premium"
        ));
        System.out.printf("Code: %s%n", result.getCode());
        System.out.printf("Message: %s%n", result.getMessage());
        System.out.printf("Output: %s%n", result.getOutput());
        System.out.printf("Duration: %dµs%n", result.getDurationUs());

        // List rulesets
        List<RuleSetSummary> rulesets = client.listRuleSets();
        for (RuleSetSummary rs : rulesets) {
            System.out.printf("  %s v%s (%d steps)%n", rs.getName(), rs.getVersion(), rs.getStepCount());
        }

        // Health check
        HealthStatus health = client.health();
        System.out.printf("Server: %s, %d rulesets%n", health.getStatus(), health.getRulesetCount());

        client.close();
    }
}
