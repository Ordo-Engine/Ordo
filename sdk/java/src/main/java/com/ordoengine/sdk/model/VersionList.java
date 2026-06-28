package com.ordoengine.sdk.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;

@JsonIgnoreProperties(ignoreUnknown = true)
public class VersionList {
    private String name;

    @JsonProperty("current_version")
    private String currentVersion;

    private List<VersionInfo> versions;

    public VersionList() {}

    public String getName() { return name; }
    public String getCurrentVersion() { return currentVersion; }
    public List<VersionInfo> getVersions() { return versions; }
}
