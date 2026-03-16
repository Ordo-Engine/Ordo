package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import java.util.Map;

@JsonIgnoreProperties(ignoreUnknown = true)
public class RuleSet {
    private RuleSetConfig config;
    private Map<String, Object> steps;

    public RuleSet() {}

    public RuleSetConfig getConfig() { return config; }
    public Map<String, Object> getSteps() { return steps; }
}
