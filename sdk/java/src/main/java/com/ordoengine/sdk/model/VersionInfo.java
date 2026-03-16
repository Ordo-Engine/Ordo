package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

@JsonIgnoreProperties(ignoreUnknown = true)
public class VersionInfo {
    private int seq;
    private String version;
    private String timestamp;

    public VersionInfo() {}

    public int getSeq() { return seq; }
    public String getVersion() { return version; }
    public String getTimestamp() { return timestamp; }
}
