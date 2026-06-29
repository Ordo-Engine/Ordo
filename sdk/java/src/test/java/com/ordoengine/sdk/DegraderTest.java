package com.ordoengine.sdk;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.ordoengine.sdk.config.DegradationConfig;
import com.ordoengine.sdk.config.DegradationMode;
import com.ordoengine.sdk.degrade.Degrader;
import com.ordoengine.sdk.model.EvalResult;
import com.ordoengine.sdk.model.ExecuteResult;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

class DegraderTest {

    private final ObjectMapper mapper = new ObjectMapper();

    private ExecuteResult execResult(String code) throws Exception {
        return mapper.readValue(
                "{\"code\":\"" + code + "\",\"message\":\"ok\",\"output\":{\"score\":1},\"duration_us\":5}",
                ExecuteResult.class);
    }

    private EvalResult evalResult(int value) throws Exception {
        return mapper.readValue(
                "{\"result\":" + value + ",\"parsed\":\"1 + 41\"}",
                EvalResult.class);
    }

    @Test
    void staleServesCachedResultFlaggedStale() throws Exception {
        Degrader d = new Degrader(DegradationConfig.builder().mode(DegradationMode.STALE).build());
        d.storeExecute("loan", Map.of("amount", 100), execResult("APPROVE"));

        ExecuteResult degraded = d.executeFallback("loan", Map.of("amount", 100));
        assertNotNull(degraded, "expected a cached result to be served");
        assertEquals("APPROVE", degraded.getCode());
        assertTrue(degraded.isStale(), "served result must be flagged stale");
    }

    @Test
    void failModeServesNothing() throws Exception {
        Degrader d = new Degrader(DegradationConfig.builder().mode(DegradationMode.FAIL).build());
        d.storeExecute("loan", Map.of("amount", 100), execResult("APPROVE"));
        assertNull(d.executeFallback("loan", Map.of("amount", 100)),
                "FAIL must never serve a fallback");
    }

    @Test
    void staleMissesOnDifferentInput() throws Exception {
        Degrader d = new Degrader(DegradationConfig.builder().mode(DegradationMode.STALE).build());
        d.storeExecute("loan", Map.of("amount", 100), execResult("APPROVE"));
        assertNull(d.executeFallback("loan", Map.of("amount", 999)),
                "different input must not hit the cache");
    }

    @Test
    void failOpenReturnsFallbackFlaggedStale() throws Exception {
        ExecuteResult fallback = execResult("DEFAULT_DENY");
        Degrader d = new Degrader(DegradationConfig.builder()
                .mode(DegradationMode.FAIL_OPEN)
                .fallback(fallback)
                .build());

        ExecuteResult degraded = d.executeFallback("loan", Map.of("amount", 100));
        assertNotNull(degraded);
        assertEquals("DEFAULT_DENY", degraded.getCode());
        assertTrue(degraded.isStale());
    }

    @Test
    void failOpenWithoutFallbackServesNothing() {
        Degrader d = new Degrader(DegradationConfig.builder().mode(DegradationMode.FAIL_OPEN).build());
        assertNull(d.executeFallback("loan", Map.of("amount", 100)));
    }

    @Test
    void evalDegradation() throws Exception {
        Degrader d = new Degrader(DegradationConfig.builder().mode(DegradationMode.STALE).build());
        d.storeEval("1 + 41", null, evalResult(42));

        EvalResult degraded = d.evalFallback("1 + 41", null);
        assertNotNull(degraded);
        assertEquals(42, ((Number) degraded.getResult()).intValue());
        assertTrue(degraded.isStale());
    }

    @Test
    void diskPersistenceSurvivesRestart(@TempDir Path tmp) throws Exception {
        String path = tmp.resolve("snapshot.ser").toString();

        Degrader d1 = new Degrader(DegradationConfig.builder()
                .mode(DegradationMode.STALE)
                .diskPath(path)
                .build());
        d1.storeExecute("loan", Map.of("amount", 100), execResult("APPROVE"));

        // New degrader reading the same on-disk cache (simulates a restart).
        Degrader d2 = new Degrader(DegradationConfig.builder()
                .mode(DegradationMode.STALE)
                .diskPath(path)
                .build());
        ExecuteResult degraded = d2.executeFallback("loan", Map.of("amount", 100));
        assertNotNull(degraded);
        assertEquals("APPROVE", degraded.getCode());
        assertTrue(degraded.isStale());
    }
}
