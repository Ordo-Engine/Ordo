package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class HealthStatus {
    private String status;
    private String version;

    @JsonProperty("ruleset_count")
    private int rulesetCount;

    @JsonProperty("uptime_seconds")
    private long uptimeSeconds;

    private StorageStatus storage;

    public HealthStatus() {}

    public String getStatus() { return status; }
    public String getVersion() { return version; }
    public int getRulesetCount() { return rulesetCount; }
    public long getUptimeSeconds() { return uptimeSeconds; }
    public StorageStatus getStorage() { return storage; }
}
