//! Joined producer shutdown independent of the HID handle lifetime.
//!
//! A barrier for task termination, not firmware admission, a cooling hold,
//! persistence success, or reconnect verification. Quiescence is one-way.
use super::Shutdown;
use tokio::{sync::watch, task::JoinHandle};

const LOST_JOIN: &str = "keeper join owner closed without completion";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuiesceError {
    #[error("keeper cleanup failed: {0}")]
    Cleanup(String),
    #[error("settings not ready: pending={pending}, last_error={last_error:?}")]
    Persistence {
        pending: bool,
        last_error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Running,
    Stopping,
    Joined,
    Failed(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct KeeperLifecycle {
    stop: watch::Sender<bool>,
    joined: watch::Receiver<State>,
}
impl KeeperLifecycle {
    pub fn state(&self) -> State {
        match self.joined.borrow().clone() {
            State::Running if self.joined.has_changed().is_err() => {
                State::Failed(vec![LOST_JOIN.into()])
            }
            State::Running if *self.stop.borrow() => State::Stopping,
            state => state,
        }
    }
    /// Close producer admission through shutdown and wait for all owned tasks.
    /// Dropping this future does not undo shutdown or detach their join owner.
    pub async fn quiesce(&self) -> Result<(), String> {
        self.stop.send_replace(true);
        let mut joined = self.joined.clone();
        loop {
            match joined.borrow().clone() {
                State::Joined => return Ok(()),
                State::Failed(errors) => return Err(errors.join("; ")),
                State::Running | State::Stopping => (),
            }
            joined.changed().await.map_err(|_| LOST_JOIN.to_owned())?;
        }
    }
}

pub(super) fn channel(
    initially_stopped: bool,
) -> (KeeperLifecycle, Shutdown, watch::Sender<State>) {
    let (stop, shutdown) = watch::channel(initially_stopped);
    let (complete, joined) = watch::channel(State::Running);
    (KeeperLifecycle { stop, joined }, shutdown, complete)
}

/// Forward manager/root shutdown (including sender drop) without coupling HID
/// termination to an explicit keeper-only quiesce request.
pub(super) fn forward(mut root: Shutdown, lifecycle: &KeeperLifecycle) -> JoinHandle<()> {
    let stop = lifecycle.stop.clone();
    tokio::spawn(async move {
        if !*root.borrow() && root.has_changed().is_ok() {
            let _ = root.changed().await;
        }
        stop.send_replace(true);
    })
}
pub(super) async fn join(name: &str, task: JoinHandle<()>, errors: &mut Vec<String>) {
    if let Err(error) = task.await {
        errors.push(format!("{name}: {error}"));
    }
}
pub(super) fn complete(sender: watch::Sender<State>, errors: Vec<String>) {
    sender.send_replace(if errors.is_empty() {
        State::Joined
    } else {
        State::Failed(errors)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn join_failure_is_retained_for_repeated_waiters_instead_of_reporting_ready() {
        let (lifecycle, _shutdown, completed) = channel(false);
        let mut errors = vec![];
        join(
            "synthetic keeper",
            tokio::spawn(async {
                panic!("synthetic join failure");
            }),
            &mut errors,
        )
        .await;
        complete(completed, errors);
        for _ in 0..2 {
            assert!(lifecycle
                .quiesce()
                .await
                .unwrap_err()
                .contains("synthetic keeper"));
        }
        assert!(matches!(lifecycle.state(), State::Failed(_)));
    }
    #[tokio::test]
    async fn missing_join_completion_fails_closed() {
        let (lifecycle, _shutdown, completed) = channel(false);
        drop(completed);
        assert!(lifecycle.quiesce().await.is_err());
        assert!(matches!(lifecycle.state(), State::Failed(_)));
    }
}
