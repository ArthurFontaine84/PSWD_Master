//! Blocage des adresses IP qui multiplient les jetons ou clés invalides.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Essais invalides tolérés par fenêtre ; le suivant est refusé.
pub(crate) const MAX_FAILURES: u32 = 10;
const WINDOW: Duration = Duration::from_mins(1);
const BLOCK: Duration = Duration::from_mins(5);

#[derive(Default)]
pub(crate) struct Limiter {
    entries: HashMap<IpAddr, Entry>,
}

struct Entry {
    failures: u32,
    window_start: Instant,
    blocked_until: Option<Instant>,
}

impl Entry {
    fn new(now: Instant) -> Self {
        Self {
            failures: 0,
            window_start: now,
            blocked_until: None,
        }
    }

    fn is_blocked(&self, now: Instant) -> bool {
        self.blocked_until.is_some_and(|until| now < until)
    }

    /// Encore utile : fenêtre en cours ou blocage actif.
    fn is_active(&self, now: Instant) -> bool {
        now < self.window_start + WINDOW || self.is_blocked(now)
    }
}

impl Limiter {
    pub(crate) fn is_blocked(&self, ip: IpAddr, now: Instant) -> bool {
        self.entries
            .get(&ip)
            .is_some_and(|entry| entry.is_blocked(now))
    }

    pub(crate) fn record_failure(&mut self, ip: IpAddr, now: Instant) {
        self.entries.retain(|_, entry| entry.is_active(now));
        let entry = self.entries.entry(ip).or_insert_with(|| Entry::new(now));
        if now >= entry.window_start + WINDOW {
            *entry = Entry::new(now);
        }
        entry.failures += 1;
        if entry.failures >= MAX_FAILURES {
            entry.blocked_until = Some(now + BLOCK);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));

    fn fail(limiter: &mut Limiter, times: u32, now: Instant) {
        for _ in 0..times {
            limiter.record_failure(IP, now);
        }
    }

    #[test]
    fn tenth_failure_blocks_the_address() {
        let mut limiter = Limiter::default();
        let now = Instant::now();
        fail(&mut limiter, MAX_FAILURES - 1, now);
        assert!(!limiter.is_blocked(IP, now));
        fail(&mut limiter, 1, now);
        assert!(limiter.is_blocked(IP, now));
        assert!(!limiter.is_blocked(OTHER, now));
    }

    #[test]
    fn block_ends_after_five_minutes() {
        let mut limiter = Limiter::default();
        let now = Instant::now();
        fail(&mut limiter, MAX_FAILURES, now);
        let just_before = now + BLOCK.saturating_sub(Duration::from_millis(1));
        assert!(limiter.is_blocked(IP, just_before));
        assert!(!limiter.is_blocked(IP, now + BLOCK));
    }

    #[test]
    fn failures_are_counted_per_minute() {
        let mut limiter = Limiter::default();
        let now = Instant::now();
        fail(&mut limiter, MAX_FAILURES - 1, now);
        fail(&mut limiter, 1, now + WINDOW);
        assert!(!limiter.is_blocked(IP, now + WINDOW));
        fail(&mut limiter, MAX_FAILURES - 2, now + WINDOW);
        assert!(!limiter.is_blocked(IP, now + WINDOW));
        fail(&mut limiter, 1, now + WINDOW);
        assert!(limiter.is_blocked(IP, now + WINDOW));
    }

    #[test]
    fn stale_entries_are_forgotten() {
        let mut limiter = Limiter::default();
        let now = Instant::now();
        limiter.record_failure(OTHER, now);
        fail(&mut limiter, MAX_FAILURES, now);
        limiter.record_failure(IP, now + WINDOW);
        assert_eq!(limiter.entries.len(), 1);
        limiter.record_failure(OTHER, now + BLOCK);
        assert_eq!(limiter.entries.len(), 1);
        assert!(limiter.entries.contains_key(&OTHER));
    }
}
