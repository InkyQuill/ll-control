//! Process-local ordinary-work admission. Closing is one-way and fail-closed;
//! dropping/cancelling a drain waiter never reopens commands. Durable firmware
//! quarantine and joined actor teardown remain separate coordinator obligations.
use anyhow::{ensure, Context};
use std::sync::Arc;
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Clone, Default)]
struct State {
    closed: bool,
    active: usize,
    cleanup_error: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Admission {
    state: watch::Sender<State>,
}
impl Default for Admission {
    fn default() -> Self {
        Self {
            state: watch::channel(State::default()).0,
        }
    }
}
impl Admission {
    pub(crate) fn same_scope(&self, other: &Self) -> bool {
        self.state.same_channel(&other.state)
    }
    pub fn enter(&self) -> anyhow::Result<Lease> {
        let admitted = self.state.send_if_modified(|state| {
            if state.closed {
                return false;
            }
            state.active += 1;
            true
        });
        ensure!(
            admitted,
            "ordinary work admission closed for firmware maintenance"
        );
        Ok(Lease {
            owner: Arc::new(LeaseOwner(self.clone())),
        })
    }
    /// Idempotent seal. This does not stop actors or resolve durable quarantine.
    pub fn close(&self) -> Drain {
        self.state.send_modify(|state| state.closed = true);
        Drain {
            state: self.state.subscribe(),
        }
    }
    /// Exclusive close owner may schedule explicit cleanup after the seal.
    pub fn begin_maintenance(&self) -> anyhow::Result<Maintenance> {
        let won = self.state.send_if_modified(|state| {
            if state.closed {
                return false;
            }
            state.closed = true;
            state.active += 1;
            true
        });
        ensure!(won, "ordinary work admission already closed");
        Ok(Maintenance {
            lease: Lease {
                owner: Arc::new(LeaseOwner(self.clone())),
            },
        })
    }
    fn cleanup_failed(&self, error: impl std::fmt::Display) {
        let message = error.to_string().chars().take(2048).collect();
        self.state.send_modify(|state| {
            if state.cleanup_error.is_none() {
                state.cleanup_error = Some(message);
            }
        });
    }
    pub fn is_closed(&self) -> bool {
        self.state.borrow().closed
    }
    pub fn active(&self) -> usize {
        self.state.borrow().active
    }
}
#[derive(Debug)]
struct LeaseOwner(Admission);
impl Drop for LeaseOwner {
    fn drop(&mut self) {
        self.0.state.send_modify(|state| state.active -= 1);
    }
}
/// Clones retain the same admitted job until its last worker/monitor exits.
#[derive(Debug, Clone)]
pub struct Lease {
    owner: Arc<LeaseOwner>,
}
impl Lease {
    pub fn child(&self) -> Self {
        Self {
            owner: self.owner.clone(),
        }
    }
    pub(crate) fn cleanup(self, work: impl FnOnce() -> anyhow::Result<()> + Send + 'static) {
        let gate = self.owner.0.clone();
        let worker_lease = self.child();
        let run = move || {
            let _lease = worker_lease;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
            match result {
                Ok(Ok(())) => (),
                Ok(Err(error)) => gate.cleanup_failed(error),
                Err(payload) => {
                    gate.cleanup_failed("cleanup worker panicked");
                    std::panic::resume_unwind(payload);
                }
            }
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let worker = runtime.spawn_blocking(run);
            runtime.spawn(async move {
                let lease = self;
                if let Err(error) = worker.await {
                    lease.owner.0.cleanup_failed(error);
                }
            });
        } else {
            run();
        }
    }
}
/// Unique maintenance owner. Cancellation leaves the gate sealed; no reopen API.
#[derive(Debug)]
pub struct Maintenance {
    lease: Lease,
}
/// One-shot cleanup authority retained by the supervisor without holding a
/// drain count. Only use after the original maintenance worker has joined.
pub(crate) struct MaintenanceRecovery(Admission);
impl MaintenanceRecovery {
    pub(crate) fn resume(self) -> Maintenance {
        self.0.state.send_modify(|state| state.active += 1);
        Maintenance {
            lease: Lease {
                owner: Arc::new(LeaseOwner(self.0)),
            },
        }
    }
}
impl Maintenance {
    pub(crate) fn recovery(&self) -> MaintenanceRecovery {
        MaintenanceRecovery(self.lease.owner.0.clone())
    }
    pub(crate) fn child(&self, gate: &Admission) -> anyhow::Result<Lease> {
        ensure!(
            self.lease.owner.0.state.same_channel(&gate.state),
            "maintenance belongs to another admission scope"
        );
        Ok(self.lease.child())
    }
    pub async fn wait_checked(self) -> anyhow::Result<()> {
        let gate = self.lease.owner.0.clone();
        drop(self);
        gate.close().wait_checked().await
    }
}
#[derive(Debug)]
pub struct Drain {
    state: watch::Receiver<State>,
}
impl Drain {
    pub async fn wait_checked(mut self) -> anyhow::Result<()> {
        while self.state.borrow_and_update().active != 0 {
            self.state.changed().await?;
        }
        let error = self.state.borrow().cleanup_error.clone();
        ensure!(
            error.is_none(),
            "cleanup failed: {}",
            error.unwrap_or_default()
        );
        Ok(())
    }
    pub async fn wait(mut self) {
        while self.state.borrow_and_update().active != 0 {
            // Lease owners retain a sender until they release the last count.
            if self.state.changed().await.is_err() {
                break;
            }
        }
    }
}
/// Existing bounded worker capacity combined with a shared admission lease.
#[derive(Debug, Clone)]
pub(crate) struct WorkSlots {
    slots: Arc<Semaphore>,
    admission: Admission,
}
#[derive(Debug)]
pub(crate) struct WorkPermit {
    _slot: OwnedSemaphorePermit,
    _lease: Lease,
}
impl WorkPermit {
    pub(crate) fn lease(&self) -> Lease {
        self._lease.child()
    }
}
impl WorkSlots {
    pub(crate) fn admission(&self) -> Admission {
        self.admission.clone()
    }
    pub(crate) fn new(capacity: usize, admission: Admission) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(capacity)),
            admission,
        }
    }
    pub(crate) fn with_admission(mut self, admission: Admission) -> Self {
        self.admission = admission;
        self
    }
    pub(crate) fn try_acquire_owned(self) -> anyhow::Result<WorkPermit> {
        let lease = self.admission.enter()?;
        let slot = self
            .slots
            .try_acquire_owned()
            .context("worker capacity exhausted")?;
        Ok(WorkPermit {
            _slot: slot,
            _lease: lease,
        })
    }
    pub(crate) async fn acquire_owned(self) -> anyhow::Result<WorkPermit> {
        let lease = self.admission.enter()?;
        let slot = self.slots.acquire_owned().await?;
        Ok(WorkPermit {
            _slot: slot,
            _lease: lease,
        })
    }
    #[cfg(test)]
    pub(crate) fn available_permits(&self) -> usize {
        self.slots.available_permits()
    }
}
#[cfg(test)]
impl From<OwnedSemaphorePermit> for WorkPermit {
    fn from(slot: OwnedSemaphorePermit) -> Self {
        Self {
            _slot: slot,
            _lease: Admission::default().enter().unwrap(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cleanup_lease_survives_seal_and_checked_drain_waiter_cancellation() {
        let gate = Admission::default();
        let owner = gate.enter().unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let maintenance = gate.begin_maintenance().unwrap();
        owner.cleanup(move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
            Ok(())
        });
        ready.await.unwrap();
        let waiter = tokio::spawn(maintenance.wait_checked());
        waiter.abort();
        let _ = waiter.await;
        assert!(gate.is_closed());
        assert_eq!(gate.active(), 1);
        let waiter = tokio::spawn(gate.close().wait_checked());
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        release.send(()).unwrap();
        waiter.await.unwrap().unwrap();
        assert_eq!(gate.active(), 0);
    }
    #[tokio::test]
    async fn cleanup_io_failure_and_panic_remain_visible_after_count_reaches_zero() {
        for panic in [false, true] {
            let gate = Admission::default();
            let owner = gate.enter().unwrap();
            let maintenance = gate.begin_maintenance().unwrap();
            owner.cleanup(move || {
                if panic {
                    panic!("injected cleanup panic");
                }
                anyhow::bail!("injected cleanup I/O failure")
            });
            assert!(maintenance.wait_checked().await.is_err());
            assert_eq!(gate.active(), 0);
            assert!(gate.close().wait_checked().await.is_err());
            assert!(gate.enter().is_err());
        }
    }
    #[tokio::test]
    async fn maintenance_capability_is_unique_and_bound_to_its_original_gate() {
        let gate = Admission::default();
        let maintenance = gate.begin_maintenance().unwrap();
        assert!(gate.begin_maintenance().is_err());
        assert!(maintenance.child(&Admission::default()).is_err());
        let child = maintenance.child(&gate).unwrap();
        drop(child);
        maintenance.wait_checked().await.unwrap();
        assert!(gate.is_closed());
        assert_eq!(gate.active(), 0);
    }
    #[tokio::test]
    async fn cancelled_waiter_does_not_reopen_and_children_keep_admission_alive() {
        let gate = Admission::default();
        let lease = gate.enter().unwrap();
        let child = lease.child();
        let drain = gate.close();
        assert!(gate.enter().is_err());
        let waiter = tokio::spawn(drain.wait());
        waiter.abort();
        let _ = waiter.await;
        drop(lease);
        assert_eq!(gate.active(), 1);
        assert!(gate.is_closed());
        drop(child);
        gate.close().wait().await;
        assert_eq!(gate.active(), 0);
        assert!(gate.enter().is_err());
    }
    #[tokio::test]
    async fn dropped_requester_does_not_hide_its_blocking_worker() {
        let gate = Admission::default();
        let slots = WorkSlots::new(1, gate.clone());
        let permit = slots.clone().try_acquire_owned().unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let waiter = tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                started.send(()).unwrap();
                blocked.recv().unwrap();
            })
            .await
            .unwrap();
        });
        ready.await.unwrap();
        waiter.abort();
        let _ = waiter.await;
        assert_eq!(gate.active(), 1);
        assert_eq!(slots.available_permits(), 0);
        let drain = tokio::spawn(gate.close().wait());
        tokio::task::yield_now().await;
        assert!(!drain.is_finished());
        release.send(()).unwrap();
        drain.await.unwrap();
        assert_eq!(gate.active(), 0);
        assert!(slots.try_acquire_owned().is_err());
    }
    #[test]
    fn full_capacity_rejection_does_not_leak_admission() {
        let gate = Admission::default();
        let slots = WorkSlots::new(1, gate.clone());
        let permit = slots.clone().try_acquire_owned().unwrap();
        assert!(slots.try_acquire_owned().is_err());
        assert_eq!(gate.active(), 1);
        drop(permit);
        assert_eq!(gate.active(), 0);
    }
    #[test]
    fn close_racing_admission_never_admits_after_seal_or_loses_count() {
        for _ in 0..32 {
            let gate = Admission::default();
            let other = gate.clone();
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let copy = barrier.clone();
            let worker = std::thread::spawn(move || {
                copy.wait();
                other.enter()
            });
            barrier.wait();
            let _drain = gate.close();
            let lease = worker.join().unwrap();
            assert_eq!(gate.active(), usize::from(lease.is_ok()));
            assert!(gate.enter().is_err());
            drop(lease);
            assert_eq!(gate.active(), 0);
        }
    }
}
