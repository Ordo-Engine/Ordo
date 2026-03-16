package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class StepTrace {
    @JsonProperty("step_id")
    private String stepId;

    @JsonProperty("step_name")
    private String stepName;

    @JsonProperty("duration_us")
    private long durationUs;

    private String result;

    public StepTrace() {}

    public String getStepId() { return stepId; }
    public String getStepName() { return stepName; }
    public long getDurationUs() { return durationUs; }
    public String getResult() { return result; }
}
