package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class RollbackResult {
    private String status;
    private String name;

    @JsonProperty("from_version")
    private String fromVersion;

    @JsonProperty("to_version")
    private String toVersion;

    public RollbackResult() {}

    public String getStatus() { return status; }
    public String getName() { return name; }
    public String getFromVersion() { return fromVersion; }
    public String getToVersion() { return toVersion; }
}
