import com.ordoengine.sdk.OrdoClient;
import com.ordoengine.sdk.model.BatchResult;
import com.ordoengine.sdk.model.ExecuteResultItem;

import java.util.List;
import java.util.Map;

public class BatchExample {
    public static void main(String[] args) {
        OrdoClient client = OrdoClient.builder()
                .httpAddress("http://localhost:8080")
                .build();

        // Batch execution
        BatchResult batch = client.executeBatch("payment_check", List.of(
                Map.of("amount", 100, "user_level", "basic"),
                Map.of("amount", 5000, "user_level", "premium"),
                Map.of("amount", 50000, "user_level", "basic")
        ));

        System.out.printf("Total: %d%n", batch.getSummary().getTotal());
        System.out.printf("Success: %d%n", batch.getSummary().getSuccess());
        System.out.printf("Failed: %d%n", batch.getSummary().getFailed());

        for (int i = 0; i < batch.getResults().size(); i++) {
            ExecuteResultItem item = batch.getResults().get(i);
            String status = item.getError() != null ? "ERROR: " + item.getError() : item.getCode();
            System.out.printf("  [%d] %s: %s%n", i, status, item.getMessage());
        }

        client.close();
    }
}
