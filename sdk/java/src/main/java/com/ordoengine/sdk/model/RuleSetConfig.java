package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class RuleSetConfig {
    private String name;
    private String version;

    @JsonProperty("entry_step")
    private String entryStep;

    public RuleSetConfig() {}

    public String getName() { return name; }
    public String getVersion() { return version; }
    public String getEntryStep() { return entryStep; }
}
