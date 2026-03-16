package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import java.util.List;

@JsonIgnoreProperties(ignoreUnknown = true)
public class BatchResult {
    private List<ExecuteResultItem> results;
    private BatchSummary summary;

    public BatchResult() {}

    public List<ExecuteResultItem> getResults() { return results; }
    public BatchSummary getSummary() { return summary; }
}
