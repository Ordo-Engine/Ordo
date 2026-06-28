package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

import java.io.Serializable;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ExecuteResult implements Serializable {
    private static final long serialVersionUID = 1L;

    private String code;
    private String message;
    private Object output;

    @JsonProperty("duration_us")
    private long durationUs;

    private ExecutionTrace trace;

    private boolean stale;

    public ExecuteResult() {}

    public String getCode() { return code; }
    public String getMessage() { return message; }
    public Object getOutput() { return output; }
    public long getDurationUs() { return durationUs; }
    public ExecutionTrace getTrace() { return trace; }

    /**
     * Whether this result was served from the local snapshot cache or a
     * configured fallback because the engine was unreachable. Always false for
     * fresh results returned by the engine.
     */
    public boolean isStale() { return stale; }

    /** Returns a copy of this result flagged as stale (degraded). */
    public ExecuteResult asStale() {
        ExecuteResult copy = new ExecuteResult();
        copy.code = this.code;
        copy.message = this.message;
        copy.output = this.output;
        copy.durationUs = this.durationUs;
        copy.trace = this.trace;
        copy.stale = true;
        return copy;
    }
}
