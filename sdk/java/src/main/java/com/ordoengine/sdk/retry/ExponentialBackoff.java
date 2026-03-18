package com.ordoengine.sdk.retry;

import java.util.concurrent.ThreadLocalRandom;

public class ExponentialBackoff {
    private final long initialMs;
    private final long maxMs;
    private final boolean jitter;

    public ExponentialBackoff(long initialMs, long maxMs, boolean jitter) {
        this.initialMs = initialMs;
        this.maxMs = maxMs;
        this.jitter = jitter;
    }

    public long nextDelay(int attempt) {
        long delay = (long) (initialMs * Math.pow(2, attempt));
        delay = Math.min(delay, maxMs);
        if (jitter) {
            double factor = 1.0 + ThreadLocalRandom.current().nextDouble(-0.3, 0.3);
            delay = Math.max(0, (long) (delay * factor));
        }
        return delay;
    }
}
