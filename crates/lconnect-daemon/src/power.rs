//! logind sleep notifications. Never requests suspend or hibernate.
//!
//! A delay inhibitor is held while awake, released after joined device teardown,
//! and reacquired on wake. Connections are pinned to a logind owner so a service
//! restart cannot silently reuse an invalid inhibitor. Reconnect reconciles the
//! current property and reopens devices after an unobserved interval. Buffered
//! sleep/wake signals retain their order even if the property is already awake.

use crate::manager::ManagerHandle;
use anyhow::{Context, Result};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::watch;
use zbus::{proxy::CacheProperties, zvariant::OwnedFd, Connection, Proxy};

const SERVICE: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const INTERFACE: &str = "org.freedesktop.login1.Manager";
const RPC_TIMEOUT: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(2);

/// Monitor the system bus until shutdown. Failure leaves ordinary daemon work
/// running, or devices paused if sleep was already observed, until the next
/// verified logind state. No guessed wake is sent when the bus is unavailable.
pub fn spawn(
    manager: ManagerHandle,
    shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(manager, shutdown, Connection::system))
}

async fn run<F, C>(manager: ManagerHandle, mut shutdown: watch::Receiver<bool>, mut connect: C)
where
    F: std::future::Future<Output = zbus::Result<Connection>>,
    C: FnMut() -> F,
{
    let mut resync = false;
    loop {
        if *shutdown.borrow() || shutdown.has_changed().is_err() {
            break;
        }
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            result = async {
                let connection = tokio::time::timeout(RPC_TIMEOUT, connect()).await??;
                monitor_connection(&connection, &manager, resync).await
            } => {
                if let Err(error) = result {
                    tracing::warn!("logind sleep monitoring unavailable: {error:#}; retrying");
                }
                // We may have missed a complete sleep/wake while disconnected.
                resync = true;
            }
        }
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = tokio::time::sleep(RETRY) => {}
        }
    }
}

async fn monitor_connection(
    connection: &Connection,
    manager: &ManagerHandle,
    resync: bool,
) -> Result<()> {
    let proxy: Proxy<'_> = tokio::time::timeout(RPC_TIMEOUT, async {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        let owner = bus.get_name_owner(SERVICE.try_into()?).await?;
        zbus::proxy::Builder::new(connection)
            .destination(owner)?
            .path(PATH)?
            .interface(INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
    })
    .await
    .context("logind owner lookup timed out")??;
    let mut signals = tokio::time::timeout(RPC_TIMEOUT, proxy.receive_signal("PrepareForSleep"))
        .await
        .context("logind subscription timed out")??;
    let max_delay: u64 =
        tokio::time::timeout(RPC_TIMEOUT, proxy.get_property("InhibitDelayMaxUSec"))
            .await
            .context("logind delay lookup timed out")??;
    // logind enforces its own deadline from signal emission. Our shorter wait
    // starts when handling the event; scheduling/queued events can consume some
    // of logind's budget already. A blocking HID syscall cannot be cancelled.
    let budget = Duration::from_micros(max_delay)
        .saturating_sub(Duration::from_millis(200))
        .min(Duration::from_secs(10));
    let mut inhibitor = acquire_inhibitor(&proxy).await;
    let mut previous = None;
    if resync {
        apply_state(true, manager, &proxy, &mut inhibitor, budget).await?;
        previous = Some(true);
    }
    let initial = read_state(connection, &proxy).await?;
    if previous != Some(initial) {
        apply_state(initial, manager, &proxy, &mut inhibitor, budget).await?;
    }
    previous = Some(initial);
    let mut health = tokio::time::interval(Duration::from_secs(1));
    health.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        // Read-only health calls are cancelled if a signal arrives, so a slow
        // property/owner reply cannot consume the sleep preparation budget.
        let sleeping = tokio::select! {
            biased;
            signal = signals.next() => {
                let signal = signal.context("logind signal stream ended")?;
                let (sleeping,): (bool,) = signal.body().deserialize()?;
                sleeping
            }
            state = async {
                health.tick().await;
                read_state(connection, &proxy).await
            } => state?,
        };
        if previous != Some(sleeping) {
            apply_state(sleeping, manager, &proxy, &mut inhibitor, budget).await?;
            previous = Some(sleeping);
        } else if !sleeping && inhibitor.is_none() {
            inhibitor = acquire_inhibitor(&proxy).await;
        }
    }
}

async fn read_state(connection: &Connection, proxy: &Proxy<'_>) -> Result<bool> {
    let owner = tokio::time::timeout(RPC_TIMEOUT, async {
        let bus = zbus::fdo::DBusProxy::new(connection).await?;
        Ok::<_, anyhow::Error>(bus.get_name_owner(SERVICE.try_into()?).await?)
    })
    .await
    .context("logind owner verification timed out")??;
    anyhow::ensure!(
        owner.as_str() == proxy.destination().as_str(),
        "logind owner changed"
    );
    Ok(
        tokio::time::timeout(RPC_TIMEOUT, proxy.get_property("PreparingForSleep"))
            .await
            .context("logind state lookup timed out")??,
    )
}

async fn apply_state(
    sleeping: bool,
    manager: &ManagerHandle,
    proxy: &Proxy<'_>,
    inhibitor: &mut Option<OwnedFd>,
    budget: Duration,
) -> Result<()> {
    if sleeping {
        let result = tokio::time::timeout(budget, manager.prepare_for_sleep(true)).await;
        match result {
            Ok(result) => result?,
            Err(_) => tracing::warn!(
                "sleep teardown exceeded inhibitor budget; device join continues in manager"
            ),
        }
        drop(inhibitor.take());
    } else {
        if inhibitor.is_none() {
            *inhibitor = acquire_inhibitor(proxy).await;
        }
        manager.prepare_for_sleep(false).await?;
    }
    Ok(())
}

async fn acquire_inhibitor(proxy: &Proxy<'_>) -> Option<OwnedFd> {
    let result = tokio::time::timeout(
        RPC_TIMEOUT,
        proxy.call::<_, _, OwnedFd>(
            "Inhibit",
            &(
                "sleep",
                "lconnectd",
                "Finish LCD writes and save settings",
                "delay",
            ),
        ),
    )
    .await;
    match result {
        Ok(Ok(fd)) => Some(fd),
        error => {
            tracing::warn!(
                ?error,
                "sleep delay inhibitor unavailable; preparation is best effort"
            );
            None
        }
    }
}

#[cfg(test)]
#[path = "power_tests.rs"]
mod tests;
