package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class BatchSummary {
    private int total;
    private int success;
    private int failed;

    @JsonProperty("total_duration_us")
    private long totalDurationUs;

    public BatchSummary() {}

    public int getTotal() { return total; }
    public int getSuccess() { return success; }
    public int getFailed() { return failed; }
    public long getTotalDurationUs() { return totalDurationUs; }
}
