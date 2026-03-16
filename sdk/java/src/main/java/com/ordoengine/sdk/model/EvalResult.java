package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

@JsonIgnoreProperties(ignoreUnknown = true)
public class EvalResult {
    private Object result;
    private String parsed;

    public EvalResult() {}

    public Object getResult() { return result; }
    public String getParsed() { return parsed; }
}
