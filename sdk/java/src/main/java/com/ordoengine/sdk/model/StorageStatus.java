package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class StorageStatus {
    private String mode;

    @JsonProperty("rules_dir")
    private String rulesDir;

    @JsonProperty("rules_count")
    private int rulesCount;

    public StorageStatus() {}

    public String getMode() { return mode; }
    public String getRulesDir() { return rulesDir; }
    public int getRulesCount() { return rulesCount; }
}
