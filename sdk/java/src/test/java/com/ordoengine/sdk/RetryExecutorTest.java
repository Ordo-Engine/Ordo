package com.ordoengine.sdk;

import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.exception.ApiException;
import com.ordoengine.sdk.retry.RetryExecutor;
import org.junit.jupiter.api.Test;

import java.time.Duration;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;

class RetryExecutorTest {

    @Test
    void succeedsFirstTry() {
        RetryExecutor executor = new RetryExecutor(RetryConfig.defaults());
        String result = executor.execute(() -> "ok");
        assertEquals("ok", result);
    }

    @Test
    void retriesOnServerError() {
        AtomicInteger attempts = new AtomicInteger(0);
        RetryExecutor executor = new RetryExecutor(
                RetryConfig.builder()
                        .maxAttempts(3)
                        .initialInterval(Duration.ofMillis(10))
                        .jitter(false)
                        .build()
        );

        String result = executor.execute(() -> {
            if (attempts.incrementAndGet() < 3) {
                throw new ApiException("server error", null, 500);
            }
            return "ok";
        });

        assertEquals("ok", result);
        assertEquals(3, attempts.get());
    }

    @Test
    void exhaustsRetries() {
        RetryExecutor executor = new RetryExecutor(
                RetryConfig.builder()
                        .maxAttempts(2)
                        .initialInterval(Duration.ofMillis(10))
                        .jitter(false)
                        .build()
        );

        assertThrows(ApiException.class, () ->
                executor.execute(() -> {
                    throw new ApiException("server error", null, 500);
                })
        );
    }

    @Test
    void doesNotRetryClientError() {
        AtomicInteger attempts = new AtomicInteger(0);
        RetryExecutor executor = new RetryExecutor(
                RetryConfig.builder()
                        .maxAttempts(3)
                        .initialInterval(Duration.ofMillis(10))
                        .build()
        );

        assertThrows(ApiException.class, () ->
                executor.execute(() -> {
                    attempts.incrementAndGet();
                    throw new ApiException("not found", null, 404);
                })
        );
        assertEquals(1, attempts.get());
    }

    @Test
    void retries429RateLimit() {
        AtomicInteger attempts = new AtomicInteger(0);
        RetryExecutor executor = new RetryExecutor(
                RetryConfig.builder()
                        .maxAttempts(3)
                        .initialInterval(Duration.ofMillis(10))
                        .jitter(false)
                        .build()
        );

        String result = executor.execute(() -> {
            if (attempts.incrementAndGet() < 2) {
                throw new ApiException("rate limited", null, 429);
            }
            return "ok";
        });

        assertEquals("ok", result);
        assertEquals(2, attempts.get());
    }
}
