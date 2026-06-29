package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

import java.io.Serializable;

@JsonIgnoreProperties(ignoreUnknown = true)
public class EvalResult implements Serializable {
    private static final long serialVersionUID = 1L;

    private Object result;
    private String parsed;

    private boolean stale;

    public EvalResult() {}

    public Object getResult() { return result; }
    public String getParsed() { return parsed; }

    /**
     * Whether this result was served from the local snapshot cache or a
     * configured fallback because the engine was unreachable.
     */
    public boolean isStale() { return stale; }

    /** Returns a copy of this result flagged as stale (degraded). */
    public EvalResult asStale() {
        EvalResult copy = new EvalResult();
        copy.result = this.result;
        copy.parsed = this.parsed;
        copy.stale = true;
        return copy;
    }
}
