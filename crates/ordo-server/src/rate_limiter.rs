//! Tenant rate limiter (token bucket)

use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct TokenBucket {
    capacity: u32,
    tokens: AtomicU32,
    refill_rate: u32,
    last_refill_ms: AtomicU64,
}

impl TokenBucket {
    pub fn new(refill_rate: u32, capacity: u32) -> Self {
        let now_ms = now_millis();
        Self {
            capacity,
            tokens: AtomicU32::new(capacity),
            refill_rate,
            last_refill_ms: AtomicU64::new(now_ms),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.refill();
        let mut current = self.tokens.load(Ordering::Relaxed);
        loop {
            if current == 0 {
                return false;
            }
            match self.tokens.compare_exchange(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    fn refill(&self) {
        if self.refill_rate == 0 || self.capacity == 0 {
            return;
        }
        let now_ms = now_millis();
        let last = self.last_refill_ms.load(Ordering::Relaxed);
        if now_ms <= last {
            return;
        }
        let elapsed_ms = now_ms - last;
        let tokens_to_add = (elapsed_ms as u128 * self.refill_rate as u128 / 1000) as u32;
        if tokens_to_add == 0 {
            return;
        }

        let mut current = self.tokens.load(Ordering::Relaxed);
        loop {
            let new_tokens = (current + tokens_to_add).min(self.capacity);
            match self.tokens.compare_exchange(
                current,
                new_tokens,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.last_refill_ms.store(now_ms, Ordering::Relaxed);
                    return;
                }
                Err(actual) => current = actual,
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RateLimiter {
    buckets: DashMap<String, TokenBucket>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: DashMap::new(),
        }
    }

    pub fn allow(&self, tenant_id: &str, qps_limit: Option<u32>, burst_limit: Option<u32>) -> bool {
        let qps = match qps_limit {
            Some(0) => return false,
            Some(v) => v,
            None => return true,
        };
        let burst = burst_limit.unwrap_or(qps).max(1);

        // Check if bucket exists and has matching config
        if let Some(bucket) = self.buckets.get(tenant_id) {
            if bucket.refill_rate == qps && bucket.capacity == burst {
                return bucket.try_acquire();
            }
            // Config changed, need to recreate bucket
            drop(bucket);
            self.buckets.remove(tenant_id);
        }

        let bucket = self
            .buckets
            .entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucket::new(qps, burst));
        bucket.try_acquire()
    }

    /// Remove a tenant's rate limit bucket (e.g., when tenant is deleted or config changes)
    pub fn remove_tenant(&self, tenant_id: &str) {
        self.buckets.remove(tenant_id);
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allow() {
        let limiter = RateLimiter::new();
        let allowed = limiter.allow("tenant-a", Some(2), Some(2));
        assert!(allowed);
    }

    #[test]
    fn test_rate_limiter_unlimited() {
        let limiter = RateLimiter::new();
        assert!(limiter.allow("tenant-a", None, None));
    }

    #[test]
    fn test_rate_limiter_burst() {
        let limiter = RateLimiter::new();
        // Burst limit of 3, QPS of 2
        assert!(limiter.allow("tenant-b", Some(2), Some(3)));
        assert!(limiter.allow("tenant-b", Some(2), Some(3)));
        assert!(limiter.allow("tenant-b", Some(2), Some(3)));
        // Should be rate limited after burst
        assert!(!limiter.allow("tenant-b", Some(2), Some(3)));
    }

    #[test]
    fn test_rate_limiter_zero_qps() {
        let limiter = RateLimiter::new();
        assert!(!limiter.allow("tenant-c", Some(0), Some(10)));
    }
}
