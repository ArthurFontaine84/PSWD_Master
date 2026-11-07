//! Verrouillage automatique : la session est détruite à l'échéance, même si l'interface attend
//! une saisie.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

struct State<S> {
    session: Option<S>,
    /// `None` : pas d'échéance.
    deadline: Option<Instant>,
    closed: bool,
}

type Shared<S> = Arc<(Mutex<State<S>>, Condvar)>;

/// Garde une session et la détruit après `timeout` sans utilisation.
pub struct IdleLock<S> {
    shared: Shared<S>,
    timeout: Duration,
}

impl<S: Send + 'static> IdleLock<S> {
    /// Un délai nul désactive le verrouillage par inactivité.
    pub fn new(session: S, timeout: Duration) -> Self {
        Self::notifying(session, timeout, || {})
    }

    /// Comme [`IdleLock::new`] ; `on_lock` est appelé une fois la session détruite à
    /// l'échéance, jamais après [`IdleLock::close`].
    pub fn notifying(
        session: S,
        timeout: Duration,
        on_lock: impl FnOnce() + Send + 'static,
    ) -> Self {
        let shared: Shared<S> = Arc::new((
            Mutex::new(State {
                session: Some(session),
                deadline: deadline_after(timeout),
                closed: false,
            }),
            Condvar::new(),
        ));
        let watched = Arc::clone(&shared);
        thread::spawn(move || {
            if watch(&watched) {
                on_lock();
            }
        });
        Self { shared, timeout }
    }

    /// Utilise la session et repousse l'échéance ; `None` si elle a été verrouillée.
    pub fn with<R>(&self, use_session: impl FnOnce(&mut S) -> R) -> Option<R> {
        let mut state = lock(&self.shared.0);
        let result = state.session.as_mut().map(use_session);
        state.deadline = deadline_after(self.timeout);
        result
    }

    /// Comme [`IdleLock::with`], sans repousser l'échéance : pour un travail qui n'est pas une
    /// activité de l'utilisateur, comme une synchro.
    pub fn quietly<R>(&self, use_session: impl FnOnce(&mut S) -> R) -> Option<R> {
        lock(&self.shared.0).session.as_mut().map(use_session)
    }

    /// Avance l'échéance : sans utilisation d'ici là, la session sera détruite dans `delay` au
    /// plus tard, même si le verrouillage par inactivité est désactivé. Ne la repousse jamais.
    pub fn expire_within(&self, delay: Duration) {
        let mut state = lock(&self.shared.0);
        let soon = Instant::now() + delay;
        state.deadline = Some(state.deadline.map_or(soon, |deadline| deadline.min(soon)));
        self.shared.1.notify_all();
    }

    /// Arrête la surveillance et rend la session, si elle n'a pas été verrouillée.
    #[must_use]
    pub fn close(self) -> Option<S> {
        let mut state = lock(&self.shared.0);
        state.closed = true;
        self.shared.1.notify_all();
        state.session.take()
    }
}

/// Abandonné sans [`IdleLock::close`], il détruit la session et arrête la surveillance.
impl<S> Drop for IdleLock<S> {
    fn drop(&mut self) {
        let mut state = lock(&self.shared.0);
        state.closed = true;
        state.session = None;
        self.shared.1.notify_all();
    }
}

/// Dort jusqu'à l'échéance ; une utilisation l'a peut-être repoussée entre-temps. Renvoie
/// `true` si la session a été détruite à l'échéance, `false` si l'`IdleLock` a été fermé avant.
fn watch<S>(shared: &(Mutex<State<S>>, Condvar)) -> bool {
    let (mutex, wakeup) = shared;
    let mut state = lock(mutex);
    while !state.closed {
        let now = Instant::now();
        state = match state.deadline {
            Some(deadline) if now >= deadline => {
                // La session est détruite ici : sa clé est effacée de la mémoire.
                state.session = None;
                return true;
            }
            Some(deadline) => {
                wakeup
                    .wait_timeout(state, deadline - now)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
            None => wakeup.wait(state).unwrap_or_else(PoisonError::into_inner),
        };
    }
    false
}

/// Un délai nul désactive le verrouillage.
fn deadline_after(timeout: Duration) -> Option<Instant> {
    (!timeout.is_zero()).then(|| Instant::now() + timeout)
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
    fn quiet_use_does_not_postpone_the_lock() {
        let (guarded, log) = guarded(Duration::from_millis(200));
        for _ in 0..3 {
            thread::sleep(Duration::from_millis(50));
            assert_eq!(guarded.quietly(|_| 7), Some(7));
        }
        thread::sleep(Duration::from_millis(300));
        assert_eq!(events(&log), ["détruite"]);
        assert_eq!(guarded.quietly(|_| 7), None);
    }

    #[test]
    fn closing_stops_the_watch() {
        let (guarded, log) = guarded(Duration::from_millis(50));
        drop(guarded.close());
        thread::sleep(Duration::from_millis(200));
        assert_eq!(events(&log), ["détruite"]);
    }

    #[test]
    fn notification_follows_the_automatic_lock_only() {
        let notified = Arc::new(Mutex::new(Vec::new()));
        let notify = |label: &'static str| {
            let notified = Arc::clone(&notified);
            move || notified.lock().unwrap().push(label)
        };
        let log = Arc::new(Mutex::new(Vec::new()));
        let idle = IdleLock::notifying(
            Fake(Arc::clone(&log)),
            Duration::from_millis(30),
            notify("échue"),
        );
        let closed = IdleLock::notifying(
            Fake(Arc::clone(&log)),
            Duration::from_millis(30),
            notify("fermée"),
        );
        drop(closed.close());
        thread::sleep(Duration::from_millis(300));
        assert_eq!(*notified.lock().unwrap(), ["échue"]);
        assert_eq!(idle.with(|_| ()), None);
    }

    #[test]
    fn zero_timeout_never_locks() {
        let (guarded, log) = guarded(Duration::ZERO);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(guarded.with(|_| ()), Some(()));
        assert_eq!(events(&log), Vec::<&str>::new());
        assert!(guarded.close().is_some());
    }

    #[test]
    fn dropping_destroys_the_session() {
        let (guarded, log) = guarded(Duration::ZERO);
        drop(guarded);
        assert_eq!(events(&log), ["détruite"]);
    }

    #[test]
    fn an_earlier_deadline_locks_sooner() {
        let (guarded, log) = guarded(Duration::from_secs(60));
        guarded.expire_within(Duration::from_millis(30));
        thread::sleep(Duration::from_millis(300));
        assert_eq!(events(&log), ["détruite"]);
        assert!(guarded.close().is_none());
    }

    #[test]
    fn an_earlier_deadline_applies_even_without_timeout() {
        let (guarded, log) = guarded(Duration::ZERO);
        guarded.expire_within(Duration::ZERO);
        thread::sleep(Duration::from_millis(300));
        assert_eq!(events(&log), ["détruite"]);
    }

    #[test]
    fn a_later_deadline_does_not_postpone_the_lock() {
        let (guarded, log) = guarded(Duration::from_millis(30));
        guarded.expire_within(Duration::from_secs(60));
        thread::sleep(Duration::from_millis(300));
        assert_eq!(events(&log), ["détruite"]);
    }

    #[test]
    fn use_after_an_earlier_deadline_restores_the_timeout() {
        let (guarded, log) = guarded(Duration::from_secs(60));
        guarded.expire_within(Duration::from_millis(100));
        assert_eq!(guarded.with(|_| ()), Some(()));
        thread::sleep(Duration::from_millis(300));
        assert_eq!(events(&log), Vec::<&str>::new());
        assert!(guarded.close().is_some());
    }
}
