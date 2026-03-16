package com.ordoengine.sdk;

import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.exception.ConfigException;
import org.junit.jupiter.api.Test;

import java.time.Duration;

import static org.junit.jupiter.api.Assertions.*;

class OrdoClientBuilderTest {

    @Test
    void defaultBuild() {
        OrdoClient client = OrdoClient.builder().build();
        assertNotNull(client);
        client.close();
    }

    @Test
    void httpOnlyBuild() {
        OrdoClient client = OrdoClient.builder()
                .httpAddress("http://localhost:9090")
                .httpOnly(true)
                .build();
        assertNotNull(client);
        client.close();
    }

    @Test
    void grpcOnlyWithoutAddressThrows() {
        assertThrows(ConfigException.class, () ->
                OrdoClient.builder().grpcOnly(true).build()
        );
    }

    @Test
    void bothOnlyModesThrows() {
        assertThrows(ConfigException.class, () ->
                OrdoClient.builder().httpOnly(true).grpcOnly(true).build()
        );
    }

    @Test
    void withRetryConfig() {
        OrdoClient client = OrdoClient.builder()
                .retry(RetryConfig.builder()
                        .maxAttempts(5)
                        .initialInterval(Duration.ofMillis(200))
                        .maxInterval(Duration.ofSeconds(10))
                        .jitter(false)
                        .build())
                .build();
        assertNotNull(client);
        client.close();
    }

    @Test
    void withTenantId() {
        OrdoClient client = OrdoClient.builder()
                .tenantId("my-tenant")
                .build();
        assertNotNull(client);
        client.close();
    }
}
