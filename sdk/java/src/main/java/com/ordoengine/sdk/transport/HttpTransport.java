package com.ordoengine.sdk.transport;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.ordoengine.sdk.exception.ApiException;
import com.ordoengine.sdk.exception.ConnectionException;
import com.ordoengine.sdk.model.*;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class HttpTransport implements AutoCloseable {
    private final String baseUrl;
    private final String apiUrl;
    private final String tenantId;
    private final HttpClient httpClient;
    private final ObjectMapper mapper;
    private final Duration timeout;

    public HttpTransport(String address, String tenantId, Duration timeout) {
        this.baseUrl = address.replaceAll("/+$", "");
        this.apiUrl = this.baseUrl + "/api/v1";
        this.tenantId = tenantId;
        this.timeout = timeout;
        this.mapper = new ObjectMapper();
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(10))
                .build();
    }

    private HttpRequest.Builder requestBuilder(String url) {
        HttpRequest.Builder builder = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .timeout(timeout)
                .header("Content-Type", "application/json");
        if (tenantId != null && !tenantId.isEmpty()) {
            builder.header("X-Tenant-ID", tenantId);
        }
        return builder;
    }

    private String doRequest(HttpRequest request) {
        try {
            HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
            if (response.statusCode() >= 400) {
                String errorMsg = response.body();
                String errorCode = null;
                try {
                    Map<String, Object> errorBody = mapper.readValue(response.body(), new TypeReference<Map<String, Object>>() {});
                    if (errorBody.containsKey("error")) {
                        errorMsg = String.valueOf(errorBody.get("error"));
                    }
                    if (errorBody.containsKey("code")) {
                        errorCode = String.valueOf(errorBody.get("code"));
                    }
                } catch (Exception ignored) {}
                throw new ApiException(errorMsg, errorCode, response.statusCode());
            }
            return response.body();
        } catch (ApiException e) {
            throw e;
        } catch (IOException e) {
            throw new ConnectionException("Failed to connect to " + request.uri(), e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new ConnectionException("Request interrupted", e);
        }
    }

    // --- Execution ---

    public ExecuteResult execute(String name, Object input, boolean includeTrace) {
        try {
            String url = apiUrl + "/execute/" + name;
            Map<String, Object> body = new HashMap<>();
            body.put("input", input);
            if (includeTrace) {
                body.put("trace", true);
            }
            HttpRequest request = requestBuilder(url)
                    .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
                    .build();
            String response = doRequest(request);
            return mapper.readValue(response, ExecuteResult.class);
        } catch (IOException e) {
            throw new ConnectionException("Serialization error", e);
        }
    }

    public BatchResult executeBatch(String name, List<?> inputs, boolean includeTrace) {
        try {
            String url = apiUrl + "/execute/" + name + "/batch";
            Map<String, Object> body = new HashMap<>();
            body.put("inputs", inputs);
            if (includeTrace) {
                Map<String, Object> options = new HashMap<>();
                options.put("trace", true);
                body.put("options", options);
            }
            HttpRequest request = requestBuilder(url)
                    .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
                    .build();
            String response = doRequest(request);
            return mapper.readValue(response, BatchResult.class);
        } catch (IOException e) {
            throw new ConnectionException("Serialization error", e);
        }
    }

    // --- Rule Management ---

    public List<RuleSetSummary> listRuleSets() {
        HttpRequest request = requestBuilder(apiUrl + "/rulesets")
                .GET()
                .build();
        String response = doRequest(request);
        try {
            return mapper.readValue(response, new TypeReference<List<RuleSetSummary>>() {});
        } catch (IOException e) {
            throw new ConnectionException("Deserialization error", e);
        }
    }

    public RuleSet getRuleSet(String name) {
        HttpRequest request = requestBuilder(apiUrl + "/rulesets/" + name)
                .GET()
                .build();
        String response = doRequest(request);
        try {
            return mapper.readValue(response, RuleSet.class);
        } catch (IOException e) {
            throw new ConnectionException("Deserialization error", e);
        }
    }

    public void createRuleSet(Object ruleset) {
        try {
            HttpRequest request = requestBuilder(apiUrl + "/rulesets")
                    .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(ruleset)))
                    .build();
            doRequest(request);
        } catch (IOException e) {
            throw new ConnectionException("Serialization error", e);
        }
    }

    public void updateRuleSet(String name, Object ruleset) {
        try {
            // Server uses POST for both create and update (upsert)
            HttpRequest request = requestBuilder(apiUrl + "/rulesets")
                    .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(ruleset)))
                    .build();
            doRequest(request);
        } catch (IOException e) {
            throw new ConnectionException("Serialization error", e);
        }
    }

    public void deleteRuleSet(String name) {
        HttpRequest request = requestBuilder(apiUrl + "/rulesets/" + name)
                .DELETE()
                .build();
        doRequest(request);
    }

    // --- Version Management ---

    public VersionList listVersions(String name) {
        HttpRequest request = requestBuilder(apiUrl + "/rulesets/" + name + "/versions")
                .GET()
                .build();
        String response = doRequest(request);
        try {
            return mapper.readValue(response, VersionList.class);
        } catch (IOException e) {
            throw new ConnectionException("Deserialization error", e);
        }
    }

    public RollbackResult rollback(String name, int seq) {
        try {
            Map<String, Object> body = new HashMap<>();
            body.put("seq", seq);
            HttpRequest request = requestBuilder(apiUrl + "/rulesets/" + name + "/rollback")
                    .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
                    .build();
            String response = doRequest(request);
            return mapper.readValue(response, RollbackResult.class);
        } catch (IOException e) {
            throw new ConnectionException("Serialization error", e);
        }
    }

    // --- Eval ---

    public EvalResult eval(String expression, Object context) {
        try {
            Map<String, Object> body = new HashMap<>();
            body.put("expression", expression);
            if (context != null) {
                body.put("context", context);
            }
            HttpRequest request = requestBuilder(apiUrl + "/eval")
                    .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
                    .build();
            String response = doRequest(request);
            return mapper.readValue(response, EvalResult.class);
        } catch (IOException e) {
            throw new ConnectionException("Serialization error", e);
        }
    }

    // --- Health ---

    public HealthStatus health() {
        HttpRequest request = requestBuilder(baseUrl + "/health")
                .GET()
                .build();
        String response = doRequest(request);
        try {
            return mapper.readValue(response, HealthStatus.class);
        } catch (IOException e) {
            throw new ConnectionException("Deserialization error", e);
        }
    }

    @Override
    public void close() {
        // HttpClient doesn't need explicit close in Java 11
    }
}
