package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class RuleSetSummary {
    private String name;
    private String version;
    private String description;

    @JsonProperty("step_count")
    private int stepCount;

    public RuleSetSummary() {}

    public String getName() { return name; }
    public String getVersion() { return version; }
    public String getDescription() { return description; }
    public int getStepCount() { return stepCount; }
}
