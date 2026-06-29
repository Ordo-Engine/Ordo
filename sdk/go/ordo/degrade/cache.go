package degrade

import (
	"encoding/json"
	"os"
	"sync"
	"time"
)

// entry is a single cached snapshot. Value holds the JSON-encoded result so the
// cache can persist to disk and stay independent of the stored result type.
type entry struct {
	Value     json.RawMessage `json:"value"`
	ExpiresAt time.Time       `json:"expires_at"`
	seq       uint64          // in-memory LRU ordering; not persisted.
}

// Cache is an in-memory TTL cache with LRU eviction and optional on-disk
// persistence so snapshots survive process restarts. It is safe for concurrent
// use.
type Cache struct {
	mu       sync.Mutex
	ttl      time.Duration
	max      int
	diskPath string
	entries  map[string]*entry
	seq      uint64
}

// NewCache creates a cache. A non-empty diskPath enables persistence: existing
// snapshots are loaded immediately and every write is flushed to the file.
func NewCache(ttl time.Duration, maxEntries int, diskPath string) *Cache {
	if ttl <= 0 {
		ttl = 5 * time.Minute
	}
	if maxEntries <= 0 {
		maxEntries = 1024
	}
	c := &Cache{
		ttl:      ttl,
		max:      maxEntries,
		diskPath: diskPath,
		entries:  make(map[string]*entry),
	}
	if diskPath != "" {
		c.load()
	}
	return c
}

// Put stores value (JSON-encoded) under key with the configured TTL.
func (c *Cache) Put(key string, value any) {
	raw, err := json.Marshal(value)
	if err != nil {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	c.seq++
	c.entries[key] = &entry{
		Value:     raw,
		ExpiresAt: time.Now().Add(c.ttl),
		seq:       c.seq,
	}
	c.evictLocked()
	if c.diskPath != "" {
		c.saveLocked()
	}
}

// Get decodes the snapshot for key into out. It reports false when the key is
// absent or expired.
func (c *Cache) Get(key string, out any) bool {
	c.mu.Lock()
	e, ok := c.entries[key]
	if ok && time.Now().After(e.ExpiresAt) {
		delete(c.entries, key)
		ok = false
	}
	if ok {
		c.seq++
		e.seq = c.seq // touch for LRU
	}
	c.mu.Unlock()

	if !ok {
		return false
	}
	return json.Unmarshal(e.Value, out) == nil
}

// evictLocked removes least-recently-used entries until within capacity.
func (c *Cache) evictLocked() {
	for len(c.entries) > c.max {
		var oldestKey string
		var oldestSeq uint64
		first := true
		for k, e := range c.entries {
			if first || e.seq < oldestSeq {
				oldestKey, oldestSeq, first = k, e.seq, false
			}
		}
		delete(c.entries, oldestKey)
	}
}

// saveLocked flushes the cache to disk. Errors are intentionally swallowed: a
// degradation cache is best-effort and must never break the caller.
func (c *Cache) saveLocked() {
	data, err := json.Marshal(c.entries)
	if err != nil {
		return
	}
	tmp := c.diskPath + ".tmp"
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		return
	}
	_ = os.Rename(tmp, c.diskPath)
}

// load reads persisted snapshots from disk, dropping any already expired.
func (c *Cache) load() {
	data, err := os.ReadFile(c.diskPath)
	if err != nil {
		return
	}
	var stored map[string]*entry
	if err := json.Unmarshal(data, &stored); err != nil {
		return
	}
	now := time.Now()
	for k, e := range stored {
		if e == nil || now.After(e.ExpiresAt) {
			continue
		}
		c.seq++
		e.seq = c.seq
		c.entries[k] = e
	}
}
