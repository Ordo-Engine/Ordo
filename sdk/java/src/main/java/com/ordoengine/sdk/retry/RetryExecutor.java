package com.ordoengine.sdk.retry;

import com.ordoengine.sdk.config.RetryConfig;
import com.ordoengine.sdk.exception.ApiException;
import com.ordoengine.sdk.exception.ConnectionException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.concurrent.Callable;

public class RetryExecutor {
    private static final Logger log = LoggerFactory.getLogger(RetryExecutor.class);
    private final RetryConfig config;
    private final ExponentialBackoff backoff;

    public RetryExecutor(RetryConfig config) {
        this.config = config;
        this.backoff = new ExponentialBackoff(
                config.getInitialInterval().toMillis(),
                config.getMaxInterval().toMillis(),
                config.isJitter()
        );
    }

    public <T> T execute(Callable<T> operation) {
        Exception lastException = null;

        for (int attempt = 0; attempt < config.getMaxAttempts(); attempt++) {
            try {
                return operation.call();
            } catch (Exception e) {
                lastException = e;
                if (!isRetryable(e)) {
                    if (e instanceof RuntimeException) {
                        throw (RuntimeException) e;
                    }
                    throw new RuntimeException(e);
                }
                if (attempt < config.getMaxAttempts() - 1) {
                    long delay = backoff.nextDelay(attempt);
                    log.debug("Retry attempt {} after {}ms: {}", attempt + 1, delay, e.getMessage());
                    try {
                        Thread.sleep(delay);
                    } catch (InterruptedException ie) {
                        Thread.currentThread().interrupt();
                        throw new RuntimeException("Retry interrupted", ie);
                    }
                }
            }
        }

        if (lastException instanceof RuntimeException) {
            throw (RuntimeException) lastException;
        }
        throw new RuntimeException("Max retry attempts exceeded", lastException);
    }

    private boolean isRetryable(Exception e) {
        if (e instanceof ApiException) {
            return ((ApiException) e).isRetryable();
        }
        if (e instanceof ConnectionException) {
            return true;
        }
        if (e instanceof java.net.http.HttpTimeoutException) {
            return true;
        }
        if (e instanceof java.io.IOException) {
            return true;
        }
        return false;
    }
}
