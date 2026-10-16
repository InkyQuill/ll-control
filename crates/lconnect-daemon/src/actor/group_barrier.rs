//! Shared admission barrier for coordinated cooling/RGB profile transactions.
//!
//! Actors acknowledge freezing only after their earlier operations and settings
//! deltas finish. The coordinator must acquire both acknowledgements before
//! committing. Ordinary mutations are rejected while held; read-only state and
//! shutdown remain available. This module does not commit or apply a profile.
use lconnect_ipc::{IpcError, Result};
use tokio::sync::watch;

/// Per-actor desired-intent revision; a fresh instance rejects pre-restart tokens.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRevision {
    instance: String,
    generation: u64,
}
impl RuntimeRevision {
    pub(crate) fn new() -> Self {
        Self {
            instance: format!("{:032x}", rand::random::<u128>()),
            generation: 0,
        }
    }
    pub(crate) fn advance(&mut self) {
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
        } else {
            *self = Self::new();
        }
    }
}

#[derive(Debug, Clone)]
pub struct GroupBarrier {
    token: u128,
    released: watch::Receiver<bool>,
}

/// Before durable commit, abandoning coordination resumes the previous runtime.
/// Seal immediately before entering commit: an unknown outcome must remain
/// frozen until explicit reconciliation or device restart.
#[derive(Debug)]
pub struct GroupBarrierOwner {
    ticket: GroupBarrier,
    release: watch::Sender<bool>,
    sealed: bool,
}
impl Default for GroupBarrierOwner {
    fn default() -> Self {
        Self::new()
    }
}
impl GroupBarrierOwner {
    pub fn new() -> Self {
        let (release, released) = watch::channel(false);
        Self {
            ticket: GroupBarrier {
                token: rand::random(),
                released,
            },
            release,
            sealed: false,
        }
    }
    pub fn ticket(&self) -> GroupBarrier {
        self.ticket.clone()
    }
    pub fn seal(&mut self) {
        self.sealed = true;
    }
    /// Call after a known rejection, or after both runtimes reconcile the
    /// committed intent. Never release an unresolved durable transaction.
    pub fn resume(mut self) {
        self.release.send_replace(true);
        self.sealed = true;
    }
}
impl Drop for GroupBarrierOwner {
    fn drop(&mut self) {
        if !self.sealed {
            self.release.send_replace(true);
        }
    }
}
impl GroupBarrier {
    pub(crate) fn held(slot: &mut Option<Self>) -> bool {
        if slot
            .as_ref()
            .is_some_and(|barrier| *barrier.released.borrow())
        {
            *slot = None;
        }
        slot.is_some()
    }
    pub(crate) fn acquire(slot: &mut Option<Self>, incoming: Self) -> Result<()> {
        if *incoming.released.borrow() {
            return Err(Self::busy());
        }
        if Self::held(slot) && slot.as_ref().is_some_and(|old| old.token != incoming.token) {
            return Err(Self::busy());
        }
        *slot = Some(incoming);
        Ok(())
    }
    pub(crate) fn owns(slot: &mut Option<Self>, ticket: &Self) -> bool {
        Self::held(slot) && slot.as_ref().is_some_and(|old| old.matches(ticket))
    }
    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.token == other.token && !*self.released.borrow() && !*other.released.borrow()
    }
    pub(crate) fn busy() -> IpcError {
        IpcError::DeviceBusy(
            "fan+lighting profile transaction in progress or requires recovery".into(),
        )
    }
}
