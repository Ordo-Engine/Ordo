package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ExecuteResultItem {
    private String code;
    private String message;
    private Object output;

    @JsonProperty("duration_us")
    private long durationUs;

    private ExecutionTrace trace;
    private String error;

    public ExecuteResultItem() {}

    public String getCode() { return code; }
    public String getMessage() { return message; }
    public Object getOutput() { return output; }
    public long getDurationUs() { return durationUs; }
    public ExecutionTrace getTrace() { return trace; }
    public String getError() { return error; }
}
