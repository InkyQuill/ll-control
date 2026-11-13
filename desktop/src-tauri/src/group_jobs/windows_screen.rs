use super::*;

/// Existing control options remain wire-compatible. Screen bindings are typed
/// separately and never interpreted as a cooling draft.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum WindowsPreparation {
    Control(WindowsOptions),
    Screen(llcontrol_ipc::WindowsScreenSource),
    Inspect(ScreenRequirementsOptions),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenRequirementsOptions {
    pub record: usize,
    pub inspect_only: bool,
}
impl Groups {
    pub fn prepare_windows_request(
        &self,
        socket: PathBuf,
        request: Start,
        options: WindowsPreparation,
    ) -> Result<Status> {
        match options {
            WindowsPreparation::Control(options) => self.prepare_windows(socket, request, options),
            WindowsPreparation::Inspect(options) => {
                ensure!(
                    options.inspect_only,
                    "Inspection options require inspect_only=true"
                );
                self.inspect_windows_screen(socket, request, options.record)
            }
            WindowsPreparation::Screen(options) => {
                self.prepare_windows_screen(socket, request, options)
            }
        }
    }
    /// Read source requirements from the same private grant selected by the chooser.
    /// This is metadata preparation, never a target grant or an asset publication.
    pub fn inspect_windows_screen(
        &self,
        socket: PathBuf,
        request: Start,
        record: usize,
    ) -> Result<Status> {
        valid_id(&request.id)?;
        ensure!(
            request.mode == Mode::WindowsScreen
                && request.group == Group::Screen
                && request.sensor_id.is_none(),
            "Invalid Windows Screen inspection target"
        );
        let (mut status, input) = {
            let state = self.lock()?;
            let job = state
                .job
                .as_ref()
                .context("Windows Screen source missing")?;
            ensure!(
                job.status.id == request.id
                    && job.status.dev == request.dev
                    && job.status.group == request.group
                    && job.status.mode == request.mode,
                "Stale Windows Screen target"
            );
            ensure!(
                *job.done.borrow() && job.status.phase == Phase::Bindings,
                "Read source requirements before preparing the upload"
            );
            (
                job.status.clone(),
                state
                    .windows
                    .as_ref()
                    .context("Windows Screen grant unavailable")?
                    .clone(),
            )
        };
        status.phase = Phase::Inspecting;
        status.windows_screen_requirements = None;
        self.launch(socket, status, true, move |owner, cancel| async move {
            let result = tokio::task::spawn_blocking(move || -> Result<_> {
                let mut file = input
                    .file
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Windows Screen grant unavailable"))?;
                file.seek(SeekFrom::Start(0))?;
                let source = llcontrol_profile::screen::inspect_windows_screen(&mut *file, record)?;
                ensure!(
                    source.archive.backup_sha256 == input.fingerprint,
                    "Windows Screen source changed"
                );
                Ok(llcontrol_profile::screen_requirements::inspect(&source)?)
            })
            .await?;
            if *cancel.borrow() {
                owner.update(&request.id, |s| s.phase = Phase::Cancelled)?;
                return Ok(());
            }
            let requirements = result?;
            owner.update(&request.id, |s| {
                s.windows_screen_requirements = Some(requirements);
                s.phase = Phase::Bindings;
            })
        })
    }
    pub fn prepare_windows_screen(
        &self,
        socket: PathBuf,
        request: Start,
        options: llcontrol_ipc::WindowsScreenSource,
    ) -> Result<Status> {
        valid_id(&request.id)?;
        ensure!(
            request.mode == Mode::WindowsScreen
                && request.group == Group::Screen
                && request.sensor_id.is_none(),
            "Invalid Windows Screen preparation target"
        );
        llcontrol_profile::linux_screen::parse_bindings(options.bindings_json.as_bytes())?;
        llcontrol_profile::screen_plan::parse_resource_map(options.resources_json.as_bytes())?;
        let (mut status, input) = {
            let state = self.lock()?;
            let job = state
                .job
                .as_ref()
                .context("Windows Screen source missing")?;
            ensure!(
                job.status.id == request.id
                    && job.status.dev == request.dev
                    && job.status.group == request.group
                    && job.status.mode == request.mode,
                "Stale Windows Screen target"
            );
            ensure!(
                *job.done.borrow() && job.status.phase == Phase::Bindings,
                "Select a new Windows Screen source before preparing again"
            );
            let input = state
                .windows
                .as_ref()
                .context("Windows Screen grant unavailable")?;
            ensure!(input.id == request.id, "Stale Windows Screen grant");
            (job.status.clone(), input.clone())
        };
        status.phase = Phase::Uploading;
        status.windows_screen_options = Some(options.clone());
        self.launch(
            socket.clone(),
            status,
            true,
            move |owner, mut cancel| async move {
                let mut file = tokio::task::spawn_blocking(move || -> Result<File> {
                    let mut grant = input
                        .file
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Windows Screen grant unavailable"))?;
                    grant.seek(SeekFrom::Start(0))?;
                    let source = llcontrol_profile::inspect_windows(&mut *grant)?;
                    ensure!(
                        source.group == llcontrol_profile::Group::Screen
                            && source.backup_sha256 == input.fingerprint,
                        "Windows Screen source changed"
                    );
                    grant.seek(SeekFrom::Start(0))?;
                    Ok(grant.try_clone()?)
                })
                .await??;
                if *cancel.borrow() {
                    owner.update(&request.id, |s| s.phase = Phase::Cancelled)?;
                    return Ok(());
                }
                let client = connect(&socket).await?;
                let snapshot = match export_rpc(&client, &request, E::Snapshot).await? {
                    ER::Snapshot { snapshot } => snapshot,
                    _ => anyhow::bail!("Expected Screen target snapshot"),
                };
                let size = tokio::task::spawn_blocking(move || -> Result<_> {
                    file.seek(SeekFrom::Start(0))?;
                    Ok((file.metadata()?.len(), file))
                })
                .await??;
                let (size, file) = size;
                owner.update(&request.id, |s| {
                    s.snapshot = Some(snapshot.clone());
                    s.phase = Phase::Uploading;
                })?;
                let result = owner
                    .upload_file(
                        &client,
                        &request,
                        file,
                        size,
                        U::BeginWindowsScreen {
                            id: request.id.clone(),
                            size,
                            expected: snapshot.expected,
                            source: options,
                        },
                        &mut cancel,
                    )
                    .await;
                match result {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        let cleanup = cleanup(&client, &request).await;
                        if *cancel.borrow() && cleanup.is_ok() {
                            owner.update(&request.id, |s| s.phase = Phase::Cancelled)?;
                            return Ok(());
                        }
                        if let Err(cleanup) = cleanup {
                            owner.update(&request.id, |s| s.phase = Phase::Uncertain)?;
                            return Err(anyhow::anyhow!(
                                "{error}; source cleanup not confirmed: {cleanup}"
                            ));
                        }
                        Err(error)
                    }
                }
            },
        )
    }
}
