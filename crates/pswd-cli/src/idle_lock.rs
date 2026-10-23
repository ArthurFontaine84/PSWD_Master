//! Verrouillage automatique : la session est détruite à l'échéance, même si le shell attend une
//! saisie.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

struct State<S> {
    session: Option<S>,
    deadline: Instant,
    closed: bool,
}

type Shared<S> = Arc<(Mutex<State<S>>, Condvar)>;

/// Garde une session et la détruit après `timeout` sans utilisation.
pub(crate) struct IdleLock<S> {
    shared: Shared<S>,
    timeout: Duration,
}

impl<S: Send + 'static> IdleLock<S> {
    /// Un délai nul désactive le verrouillage.
    pub fn new(session: S, timeout: Duration) -> Self {
        let shared: Shared<S> = Arc::new((
            Mutex::new(State {
                session: Some(session),
                deadline: Instant::now() + timeout,
                closed: false,
            }),
            Condvar::new(),
        ));
        if !timeout.is_zero() {
            let watched = Arc::clone(&shared);
            thread::spawn(move || watch(&watched));
        }
        Self { shared, timeout }
    }

    /// Utilise la session et repousse l'échéance ; `None` si elle a été verrouillée.
    pub fn with<R>(&self, use_session: impl FnOnce(&mut S) -> R) -> Option<R> {
        let mut state = lock(&self.shared.0);
        let result = state.session.as_mut().map(use_session);
        state.deadline = Instant::now() + self.timeout;
        result
    }

    /// Arrête la surveillance et rend la session, si elle n'a pas été verrouillée.
    pub fn close(self) -> Option<S> {
        let mut state = lock(&self.shared.0);
        state.closed = true;
        self.shared.1.notify_all();
        state.session.take()
    }
}

/// Dort jusqu'à l'échéance ; une utilisation l'a peut-être repoussée entre-temps.
fn watch<S>(shared: &(Mutex<State<S>>, Condvar)) {
    let (mutex, wakeup) = shared;
    let mut state = lock(mutex);
    while !state.closed {
        let now = Instant::now();
        if now >= state.deadline {
            // La session est détruite ici : sa clé est effacée de la mémoire.
            state.session = None;
            return;
        }
        let remaining = state.deadline - now;
        state = wakeup
            .wait_timeout(state, remaining)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

fn lock<S>(mutex: &Mutex<State<S>>) -> MutexGuard<'_, State<S>> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Session factice qui note ce qui lui arrive.
    struct Fake(Arc<Mutex<Vec<&'static str>>>);

    impl Drop for Fake {
        fn drop(&mut self) {
            self.0.lock().unwrap().push("détruite");
        }
    }

    fn guarded(timeout: Duration) -> (IdleLock<Fake>, Arc<Mutex<Vec<&'static str>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        (IdleLock::new(Fake(Arc::clone(&log)), timeout), log)
    }

    fn events(log: &Mutex<Vec<&'static str>>) -> Vec<&'static str> {
        log.lock().unwrap().clone()
    }

    #[test]
    fn session_is_usable_before_the_deadline() {
        let (guarded, log) = guarded(Duration::from_secs(60));
        assert_eq!(guarded.with(|_| 42), Some(42));
        assert!(guarded.close().is_some());
        assert_eq!(events(&log), ["détruite"]);
    }

    #[test]
    fn idle_session_is_locked_then_destroyed() {
        let (guarded, log) = guarded(Duration::from_millis(30));
        thread::sleep(Duration::from_millis(300));
        assert_eq!(events(&log), ["détruite"]);
        assert_eq!(guarded.with(|_| ()), None);
        assert!(guarded.close().is_none());
    }

    #[test]
    fn activity_postpones_the_lock() {
        let (guarded, log) = guarded(Duration::from_millis(200));
        for _ in 0..8 {
            thread::sleep(Duration::from_millis(50));
            assert_eq!(guarded.with(|_| ()), Some(()));
        }
        assert_eq!(events(&log), Vec::<&str>::new());
    }

    #[test]
    fn closing_stops_the_watch() {
        let (guarded, log) = guarded(Duration::from_millis(50));
        drop(guarded.close());
        thread::sleep(Duration::from_millis(200));
        assert_eq!(events(&log), ["détruite"]);
    }

    #[test]
    fn zero_timeout_never_locks() {
        let (guarded, log) = guarded(Duration::ZERO);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(guarded.with(|_| ()), Some(()));
        assert_eq!(events(&log), Vec::<&str>::new());
    }
}
