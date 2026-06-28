package com.ordoengine.sdk.degrade;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.Serializable;
import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * In-memory TTL cache with LRU eviction and optional on-disk persistence so
 * snapshots survive process restarts. Best-effort: I/O errors are swallowed so
 * the cache can never break a caller.
 */
class SnapshotCache {

    private static final class Entry implements Serializable {
        private static final long serialVersionUID = 1L;
        final Object value;
        final long expiresAtMillis;

        Entry(Object value, long expiresAtMillis) {
            this.value = value;
            this.expiresAtMillis = expiresAtMillis;
        }
    }

    private final long ttlMillis;
    private final int maxEntries;
    private final String diskPath;
    private final LinkedHashMap<String, Entry> entries;

    SnapshotCache(Duration ttl, int maxEntries, String diskPath) {
        this.ttlMillis = (ttl != null && !ttl.isZero() && !ttl.isNegative())
                ? ttl.toMillis()
                : Duration.ofMinutes(5).toMillis();
        this.maxEntries = maxEntries > 0 ? maxEntries : 1024;
        this.diskPath = diskPath;
        final int cap = this.maxEntries;
        this.entries = new LinkedHashMap<String, Entry>(16, 0.75f, true) {
            private static final long serialVersionUID = 1L;

            @Override
            protected boolean removeEldestEntry(Map.Entry<String, Entry> eldest) {
                return size() > cap;
            }
        };
        if (diskPath != null && !diskPath.isEmpty()) {
            load();
        }
    }

    synchronized Object get(String key) {
        Entry e = entries.get(key);
        if (e == null) {
            return null;
        }
        if (System.currentTimeMillis() > e.expiresAtMillis) {
            entries.remove(key);
            return null;
        }
        return e.value;
    }

    synchronized void put(String key, Object value) {
        entries.put(key, new Entry(value, System.currentTimeMillis() + ttlMillis));
        if (diskPath != null && !diskPath.isEmpty()) {
            save();
        }
    }

    private void save() {
        try (ObjectOutputStream out = new ObjectOutputStream(new FileOutputStream(diskPath))) {
            // Persist a plain copy (the live map is an anonymous LRU subclass).
            out.writeObject(new LinkedHashMap<>(entries));
        } catch (IOException ignored) {
            // best-effort
        }
    }

    @SuppressWarnings("unchecked")
    private void load() {
        File f = new File(diskPath);
        if (!f.exists()) {
            return;
        }
        try (ObjectInputStream in = new ObjectInputStream(new FileInputStream(f))) {
            Object obj = in.readObject();
            if (obj instanceof Map) {
                long now = System.currentTimeMillis();
                for (Map.Entry<String, Entry> en : ((Map<String, Entry>) obj).entrySet()) {
                    Entry e = en.getValue();
                    if (e != null && e.expiresAtMillis > now) {
                        entries.put(en.getKey(), e);
                    }
                }
            }
        } catch (IOException | ClassNotFoundException | ClassCastException ignored) {
            // best-effort: ignore a corrupt or incompatible cache file
        }
    }
}
