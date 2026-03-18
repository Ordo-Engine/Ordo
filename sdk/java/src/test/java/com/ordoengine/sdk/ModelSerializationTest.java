package com.ordoengine.sdk;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.ordoengine.sdk.model.*;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ModelSerializationTest {

    private final ObjectMapper mapper = new ObjectMapper();

    @Test
    void deserializeExecuteResult() throws Exception {
        String json = "{\"code\":\"APPROVED\",\"message\":\"OK\",\"output\":{\"score\":0.5},\"duration_us\":150}";
        ExecuteResult result = mapper.readValue(json, ExecuteResult.class);
        assertEquals("APPROVED", result.getCode());
        assertEquals("OK", result.getMessage());
        assertEquals(150, result.getDurationUs());
        assertNotNull(result.getOutput());
        assertNull(result.getTrace());
    }

    @Test
    void deserializeExecuteResultWithTrace() throws Exception {
        String json = "{\"code\":\"OK\",\"message\":\"m\",\"output\":null,\"duration_us\":10," +
                "\"trace\":{\"path\":\"a -> b\",\"steps\":[{\"step_id\":\"s1\",\"step_name\":\"check\",\"duration_us\":5,\"result\":\"true\"}]}}";
        ExecuteResult result = mapper.readValue(json, ExecuteResult.class);
        assertNotNull(result.getTrace());
        assertEquals("a -> b", result.getTrace().getPath());
        assertEquals(1, result.getTrace().getSteps().size());
        assertEquals("s1", result.getTrace().getSteps().get(0).getStepId());
    }

    @Test
    void deserializeBatchResult() throws Exception {
        String json = "{\"results\":[" +
                "{\"code\":\"OK\",\"message\":\"m\",\"output\":{},\"duration_us\":10}," +
                "{\"code\":\"ERR\",\"message\":\"fail\",\"output\":null,\"duration_us\":5,\"error\":\"timeout\"}" +
                "],\"summary\":{\"total\":2,\"success\":1,\"failed\":1,\"total_duration_us\":15}}";
        BatchResult result = mapper.readValue(json, BatchResult.class);
        assertEquals(2, result.getResults().size());
        assertEquals("OK", result.getResults().get(0).getCode());
        assertEquals("timeout", result.getResults().get(1).getError());
        assertEquals(2, result.getSummary().getTotal());
        assertEquals(1, result.getSummary().getFailed());
    }

    @Test
    void deserializeHealthStatus() throws Exception {
        String json = "{\"status\":\"healthy\",\"version\":\"0.3.0\",\"ruleset_count\":5,\"uptime_seconds\":3600," +
                "\"storage\":{\"mode\":\"file\",\"rules_dir\":\"/data\",\"rules_count\":5}}";
        HealthStatus health = mapper.readValue(json, HealthStatus.class);
        assertEquals("healthy", health.getStatus());
        assertEquals(5, health.getRulesetCount());
        assertNotNull(health.getStorage());
        assertEquals("file", health.getStorage().getMode());
    }

    @Test
    void deserializeVersionList() throws Exception {
        String json = "{\"name\":\"test\",\"current_version\":\"1.0.0\",\"versions\":[" +
                "{\"seq\":1,\"version\":\"0.9.0\",\"timestamp\":\"2024-01-01T00:00:00Z\"}," +
                "{\"seq\":2,\"version\":\"1.0.0\",\"timestamp\":\"2024-01-02T00:00:00Z\"}" +
                "]}";
        VersionList vl = mapper.readValue(json, VersionList.class);
        assertEquals("test", vl.getName());
        assertEquals("1.0.0", vl.getCurrentVersion());
        assertEquals(2, vl.getVersions().size());
    }

    @Test
    void ignoresUnknownFields() throws Exception {
        String json = "{\"code\":\"OK\",\"message\":\"m\",\"output\":null,\"duration_us\":1,\"unknown_field\":true}";
        ExecuteResult result = mapper.readValue(json, ExecuteResult.class);
        assertEquals("OK", result.getCode());
    }
}
