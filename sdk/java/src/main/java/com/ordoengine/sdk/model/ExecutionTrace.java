package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import java.util.List;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ExecutionTrace {
    private String path;
    private List<StepTrace> steps;

    public ExecutionTrace() {}

    public String getPath() { return path; }
    public List<StepTrace> getSteps() { return steps; }
}
