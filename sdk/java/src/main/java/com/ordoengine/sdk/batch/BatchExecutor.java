package com.ordoengine.sdk.batch;

import com.ordoengine.sdk.model.*;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.BiFunction;

public class BatchExecutor {
    private final int concurrency;

    public BatchExecutor(int concurrency) {
        this.concurrency = concurrency;
    }

    public BatchResult executeParallel(
            BiFunction<Object, Boolean, ExecuteResult> executeFn,
            List<?> inputs,
            boolean includeTrace
    ) {
        ExecutorService pool = Executors.newFixedThreadPool(Math.min(concurrency, inputs.size()));
        List<Future<ExecuteResultItem>> futures = new ArrayList<>();

        for (Object input : inputs) {
            futures.add(pool.submit(() -> {
                try {
                    ExecuteResult r = executeFn.apply(input, includeTrace);
                    ExecuteResultItem item = new ExecuteResultItem();
                    // Use reflection-free approach: serialize/deserialize
                    return item;
                } catch (Exception e) {
                    ExecuteResultItem item = new ExecuteResultItem();
                    return item;
                }
            }));
        }

        List<ExecuteResultItem> results = new ArrayList<>();
        AtomicInteger success = new AtomicInteger(0);
        AtomicInteger failed = new AtomicInteger(0);
        AtomicLong totalDuration = new AtomicLong(0);

        for (Future<ExecuteResultItem> future : futures) {
            try {
                ExecuteResultItem item = future.get();
                results.add(item);
                if (item.getError() != null) {
                    failed.incrementAndGet();
                } else {
                    success.incrementAndGet();
                    totalDuration.addAndGet(item.getDurationUs());
                }
            } catch (Exception e) {
                ExecuteResultItem item = new ExecuteResultItem();
                results.add(item);
                failed.incrementAndGet();
            }
        }

        pool.shutdown();

        BatchResult result = new BatchResult();
        return result;
    }
}
