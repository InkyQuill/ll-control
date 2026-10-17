#![forbid(unsafe_code)]
use lconnect_ipc::client::IpcClient;
use serde::Serialize;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
struct Bridge(Arc<Mutex<Option<Arc<IpcClient>>>>);
#[derive(Debug, Serialize)]
struct BridgeError {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_code: Option<lconnect_gui::archive_messages::ArchiveNotice>,
}
fn error(e: impl std::fmt::Display) -> BridgeError {
    BridgeError {
        message: e.to_string(),
        code: None,
        message_code: None,
    }
}
fn archive_error(e: anyhow::Error) -> BridgeError {
    BridgeError {
        message_code: lconnect_gui::archive_messages::ArchiveNotice::from_error(&e),
        message: e.to_string(),
        code: None,
    }
}
fn allowed(method: &str) -> bool {
    matches!(
        method,
        "list_devices"
            | "get_firmware"
            | "get_firmware_recovery"
            | "list_sensors"
            | "get_system_spec"
            | "get_fps_source"
            | "set_fps_source"
            | "get_display_settings"
            | "set_display_settings"
            | "get_sensor_defaults"
            | "set_sensor_defaults"
            | "reset_screen"
            | "get_screen_reset"
            | "reapply_screen_reset"
            | "get_screen_collections"
            | "set_screen_collections"
            | "migrate_screen_collections"
            | "list_profiles"
            | "edit_profile"
            | "cleanup_theme_assets"
            | "cleanup_theme_fonts"
            | "list_prepared_clips"
            | "delete_prepared_clip"
            | "get_lcd_media"
            | "set_lcd_media"
            | "get_lcd_state"
            | "get_lcd_theme"
            | "get_imported_lcd_editor"
            | "preview_windows_screen_upload"
            | "set_lcd_theme"
            | "preview_lcd_theme"
            | "preview_lcd_editor"
            | "list_named_theme_assets"
            | "rename_theme_asset"
            | "delete_theme_asset"
            | "list_theme_assets"
            | "thumbnail_theme_asset"
            | "read_theme_asset_for_edit"
            | "import_theme_asset"
            | "font_upload"
            | "patch_lcd_config"
            | "clear_lcd"
            | "get_cooling_state"
            | "get_cooling_modes"
            | "save_cooling_mode"
            | "set_cooling_policy"
            | "apply_cooling_mode"
            | "initialize_cooling_modes"
            | "get_lighting_info"
            | "set_lighting"
            | "get_dynamic_rgb"
            | "set_dynamic_rgb"
            | "get_thermal_state"
            | "set_thermal_config"
            | "daemon_stats"
    )
}
#[tauri::command]
async fn group_archive_start(
    jobs: tauri::State<'_, lconnect_gui::group_jobs::Groups>,
    request: lconnect_gui::group_jobs::Start,
    language: Option<lconnect_ipc::InterfaceLanguage>,
) -> Result<lconnect_gui::group_jobs::Status, BridgeError> {
    jobs.start(
        lconnect_ipc::socket::default_socket_path().map_err(error)?,
        request,
        language.unwrap_or_default(),
    )
    .map_err(archive_error)
}
#[tauri::command]
async fn group_windows_prepare(
    jobs: tauri::State<'_, lconnect_gui::group_jobs::Groups>,
    request: lconnect_gui::group_jobs::Start,
    options: lconnect_gui::group_jobs::WindowsPreparation,
) -> Result<lconnect_gui::group_jobs::Status, BridgeError> {
    jobs.prepare_windows_request(
        lconnect_ipc::socket::default_socket_path().map_err(error)?,
        request,
        options,
    )
    .map_err(archive_error)
}
#[tauri::command]
fn group_archive_status(
    jobs: tauri::State<'_, lconnect_gui::group_jobs::Groups>,
) -> Result<Option<lconnect_gui::group_jobs::Status>, BridgeError> {
    jobs.status().map_err(archive_error)
}
#[tauri::command]
async fn group_archive_action(
    jobs: tauri::State<'_, lconnect_gui::group_jobs::Groups>,
    request: lconnect_gui::group_jobs::Start,
    action: lconnect_gui::group_jobs::Action,
    expected: Option<String>,
) -> Result<Option<lconnect_gui::group_jobs::Status>, BridgeError> {
    jobs.action(
        lconnect_ipc::socket::default_socket_path().map_err(error)?,
        request,
        action,
        expected,
    )
    .map_err(archive_error)
}
#[tauri::command]
async fn profile_archive_start(
    jobs: tauri::State<'_, lconnect_gui::archive_jobs::Archives>,
    request: lconnect_gui::archive_jobs::Start,
    language: Option<lconnect_ipc::InterfaceLanguage>,
) -> Result<lconnect_gui::archive_jobs::Status, BridgeError> {
    jobs.start(
        lconnect_ipc::socket::default_socket_path().map_err(error)?,
        request,
        language.unwrap_or_default(),
    )
    .map_err(archive_error)
}
#[tauri::command]
fn profile_archive_status(
    jobs: tauri::State<'_, lconnect_gui::archive_jobs::Archives>,
) -> Result<Option<lconnect_gui::archive_jobs::Status>, BridgeError> {
    jobs.status().map_err(archive_error)
}
#[tauri::command]
fn profile_archive_cancel(
    jobs: tauri::State<'_, lconnect_gui::archive_jobs::Archives>,
    id: String,
) -> Result<lconnect_gui::archive_jobs::Status, BridgeError> {
    jobs.cancel(&id).map_err(archive_error)
}
#[tauri::command]
async fn profile_archive_recover(
    jobs: tauri::State<'_, lconnect_gui::archive_jobs::Archives>,
    id: String,
    action: lconnect_gui::archive_jobs::Recovery,
    group: lconnect_ipc::ProfileGroup,
    slot: u8,
) -> Result<lconnect_gui::archive_jobs::Status, BridgeError> {
    jobs.recover(
        lconnect_ipc::socket::default_socket_path().map_err(error)?,
        id,
        action,
        group,
        slot,
    )
    .map_err(archive_error)
}
#[tauri::command]
fn profile_archive_dismiss(
    jobs: tauri::State<'_, lconnect_gui::archive_jobs::Archives>,
    id: String,
) -> Result<(), BridgeError> {
    jobs.dismiss(&id).map_err(archive_error)
}

#[tauri::command]
async fn daemon_call(
    bridge: tauri::State<'_, Bridge>,
    method: String,
    params: Value,
) -> Result<Value, BridgeError> {
    if !allowed(&method) {
        return Err(error("Unsupported desktop operation"));
    }
    bridge_call(&bridge, method, params).await
}
async fn bridge_call(bridge: &Bridge, method: String, params: Value) -> Result<Value, BridgeError> {
    if serde_json::to_vec(&params).map_err(error)?.len() > 5 * 1024 * 1024 {
        return Err(error("Request too large"));
    }
    let client = {
        let mut current = bridge.0.lock().await;
        if current.is_none() {
            let path = lconnect_ipc::socket::default_socket_path().map_err(error)?;
            let connected = tokio::time::timeout(Duration::from_secs(3), IpcClient::connect(&path))
                .await
                .map_err(error)?
                .map_err(error)?;
            *current = Some(Arc::new(connected));
        }
        current
            .as_ref()
            .cloned()
            .ok_or_else(|| error("Daemon disconnected"))?
    };
    let result = tokio::time::timeout(Duration::from_secs(10), client.call(&method, params)).await;
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e @ lconnect_ipc::IpcError::InvalidArgument(_))) => Err(BridgeError {
            message: e.to_string(),
            code: Some(e.jsonrpc_code()),
            message_code: None,
        }),
        failure => {
            // Never replay writes after an ambiguous timeout. Reconnect on next request.
            let mut current = bridge.0.lock().await;
            if current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &client)) {
                *current = None;
            }
            Err(error(match failure {
                Ok(Err(e)) => e.to_string(),
                Err(e) => format!("Request timed out; applied state must be checked: {e}"),
                _ => unreachable!(),
            }))
        }
    }
}
use lconnect_gui::media_import::{MediaImports, Status as MediaStatus};
use lconnect_gui::media_prepare::Fit;
use lconnect_gui::media_source::{MediaSources, Status as SourceStatus};

use lconnect_gui::media_thumbnail::Thumbnails;
#[tauri::command]
async fn media_source_start(
    sources: tauri::State<'_, MediaSources>,
    id: String,
    key: String,
) -> Result<SourceStatus, BridgeError> {
    let socket = lconnect_ipc::socket::default_socket_path().map_err(error)?;
    sources.start(socket, id, key).map_err(error)
}
#[tauri::command]
fn media_source_status(
    sources: tauri::State<'_, MediaSources>,
) -> Result<Option<SourceStatus>, BridgeError> {
    sources.status().map_err(error)
}
#[tauri::command]
async fn media_source_cancel(
    sources: tauri::State<'_, MediaSources>,
    id: String,
) -> Result<SourceStatus, BridgeError> {
    let sources = sources.inner().clone();
    tokio::task::spawn_blocking(move || sources.cancel(&id))
        .await
        .map_err(error)?
        .map_err(error)
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Two managed states plus backward-compatible Tauri command fields"
)]
async fn media_source_prepare(
    sources: tauri::State<'_, MediaSources>,
    imports: tauri::State<'_, MediaImports>,
    source_id: String,
    id: String,
    name: String,
    fit: Fit,
    cut: Option<lconnect_gui::media_prepare::VideoCut>,
    adjustments: Option<lconnect_gui::video_adjustments::VideoAdjustments>,
    range: Option<lconnect_gui::video_range::VideoEditRange>,
) -> Result<(), BridgeError> {
    let sources = sources.inner().clone();
    let imports = imports.inner().clone();
    tauri::async_runtime::spawn(async move {
        let owner = imports.clone();
        let job = id.clone();
        tokio::task::spawn_blocking(move || {
            owner.begin_source_edited(
                &sources,
                &source_id,
                job,
                name,
                lconnect_gui::media_import::EditOptions {
                    range,
                    fit,
                    cut,
                    adjustments: adjustments.unwrap_or_default(),
                },
            )
        })
        .await
        .map_err(error)?
        .map_err(error)?;
        imports.prepare_review(&id).await.map_err(error)
    })
    .await
    .map_err(error)?
}

#[tauri::command]
async fn media_thumbnail(
    thumbnails: tauri::State<'_, Thumbnails>,
    bridge: tauri::State<'_, Bridge>,
    id: String,
    key: String,
) -> Result<Vec<u8>, BridgeError> {
    let lease = thumbnails.begin(id).map_err(error)?;
    let value = bridge_call(
        &bridge,
        "load_media_first_frame".into(),
        serde_json::json!({"key":key}),
    )
    .await?;
    let clip = lconnect_ipc::H264Clip {
        fps: 24,
        units: vec![serde_json::from_value(value).map_err(error)?],
    };
    lease.render(clip).await.map_err(error)
}
#[tauri::command]
fn media_thumbnail_cancel(
    thumbnails: tauri::State<'_, Thumbnails>,
    id: String,
) -> Result<(), BridgeError> {
    thumbnails.cancel(&id).map_err(error)
}

#[tauri::command]
async fn media_begin(
    imports: tauri::State<'_, MediaImports>,
    id: String,
    name: String,
    total: u64,
    fit: Fit,
    cut: Option<lconnect_gui::media_prepare::VideoCut>,
    adjustments: Option<lconnect_gui::video_adjustments::VideoAdjustments>,
) -> Result<MediaStatus, BridgeError> {
    let imports = imports.inner().clone();
    tokio::task::spawn_blocking(move || {
        imports.begin_edited(
            id,
            name,
            total,
            lconnect_gui::media_import::EditOptions {
                range: None,
                fit,
                cut,
                adjustments: adjustments.unwrap_or_default(),
            },
        )
    })
    .await
    .map_err(error)?
    .map_err(error)
}
#[tauri::command]
async fn media_inspect(
    imports: tauri::State<'_, MediaImports>,
    id: String,
) -> Result<MediaStatus, BridgeError> {
    let imports = imports.inner().clone();
    tauri::async_runtime::spawn(async move { imports.inspect(&id).await.map_err(error) })
        .await
        .map_err(error)?
}

#[tauri::command]
async fn media_prepare_inspected(
    imports: tauri::State<'_, MediaImports>,
    id: String,
    fit: Fit,
    cut: Option<lconnect_gui::media_prepare::VideoCut>,
    adjustments: Option<lconnect_gui::video_adjustments::VideoAdjustments>,
    range: Option<lconnect_gui::video_range::VideoEditRange>,
) -> Result<(), BridgeError> {
    let imports = imports.inner().clone();
    tauri::async_runtime::spawn(async move {
        imports
            .prepare_review_edited(
                &id,
                lconnect_gui::media_import::EditOptions {
                    range,
                    fit,
                    cut,
                    adjustments: adjustments.unwrap_or_default(),
                },
            )
            .await
            .map_err(error)
    })
    .await
    .map_err(error)?
}

#[tauri::command]
async fn media_append(
    imports: tauri::State<'_, MediaImports>,
    id: String,
    offset: u64,
    bytes: Vec<u8>,
) -> Result<MediaStatus, BridgeError> {
    let imports = imports.inner().clone();
    tokio::task::spawn_blocking(move || imports.append(&id, offset, &bytes))
        .await
        .map_err(error)?
        .map_err(error)
}
#[tauri::command]
async fn media_status(
    imports: tauri::State<'_, MediaImports>,
) -> Result<Option<MediaStatus>, BridgeError> {
    let imports = imports.inner().clone();
    tokio::task::spawn_blocking(move || imports.status())
        .await
        .map_err(error)?
        .map_err(error)
}
#[tauri::command]
async fn media_cancel(
    imports: tauri::State<'_, MediaImports>,
    id: String,
) -> Result<MediaStatus, BridgeError> {
    let imports = imports.inner().clone();
    tokio::task::spawn_blocking(move || imports.cancel(&id))
        .await
        .map_err(error)?
        .map_err(error)
}
#[tauri::command]
async fn media_start(
    review: Option<bool>,
    imports: tauri::State<'_, MediaImports>,
    bridge: tauri::State<'_, Bridge>,
    id: String,
) -> Result<(), BridgeError> {
    let reader = imports.inner().clone();
    let status = tokio::task::spawn_blocking(move || reader.status())
        .await
        .map_err(error)?
        .map_err(error)?
        .ok_or_else(|| error("No media import"))?;
    if status.id != id
        || status.phase != lconnect_gui::media_import::Phase::Uploading
        || status.received != status.total
    {
        return Err(error("Upload is incomplete or already started"));
    }
    let imports = imports.inner().clone();
    let bridge = bridge.inner().clone();
    // The owned task survives an invoke response being lost or its window unmounting.
    tauri::async_runtime::spawn(async move {
        if review.unwrap_or(false) {
            let _ = imports.prepare_review(&id).await;
        } else if let Ok(Some(prepared)) = imports.prepare(&id).await {
            let _ = imports
                .publish(&id, prepared, |request| {
                    let bridge = bridge.clone();
                    async move {
                        let value = bridge_call(
                            &bridge,
                            "media_upload".into(),
                            serde_json::json!({"request":request}),
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!(e.message))?;
                        Ok(serde_json::from_value(value)?)
                    }
                })
                .await;
        }
    });
    Ok(())
}

#[tauri::command]
async fn media_confirm(
    imports: tauri::State<'_, MediaImports>,
    bridge: tauri::State<'_, Bridge>,
    id: String,
) -> Result<(), BridgeError> {
    let imports = imports.inner().clone();
    let bridge = bridge.inner().clone();
    // Take and publish inside the owned task, so a lost invoke future cannot
    // strand an admitted prepared owner between confirmation and publication.
    tauri::async_runtime::spawn(async move {
        let owner = imports.clone();
        let job = id.clone();
        let prepared = tokio::task::spawn_blocking(move || owner.take_review(&job))
            .await
            .map_err(error)?
            .map_err(error)?;
        imports
            .publish(&id, prepared, |request| {
                let bridge = bridge.clone();
                async move {
                    let value = bridge_call(
                        &bridge,
                        "media_upload".into(),
                        serde_json::json!({"request":request}),
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!(e.message))?;
                    Ok(serde_json::from_value(value)?)
                }
            })
            .await
            .map_err(error)
    })
    .await
    .map_err(error)?
}

#[tauri::command]
async fn media_playback_url(
    server: tauri::State<'_, lconnect_gui::media_http::MediaHttp>,
    imports: tauri::State<'_, MediaImports>,
    sources: tauri::State<'_, MediaSources>,
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    kind: String,
    id: String,
) -> Result<String, BridgeError> {
    server
        .url(
            &kind,
            &id,
            imports.inner().clone(),
            sources.inner().clone(),
            recordings.inner().clone(),
        )
        .await
        .map_err(error)
}

fn with_media_preview(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    use tauri::Manager;
    let builder = builder.register_asynchronous_uri_scheme_protocol(
        "media-source",
        |context, request, responder| {
            let sources = context.app_handle().state::<MediaSources>().inner().clone();
            let window = context.webview_label().to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(lconnect_gui::media_preview::serve(
                    &sources, &window, request,
                ));
            });
        },
    );
    builder.register_asynchronous_uri_scheme_protocol(
        "media-preview",
        |context, request, responder| {
            let imports = context.app_handle().state::<MediaImports>().inner().clone();
            let window = context.webview_label().to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(lconnect_gui::media_preview::serve(
                    &imports, &window, request,
                ));
            });
        },
    )
}

#[tauri::command]
async fn screen_record_begin(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    id: String,
) -> Result<lconnect_gui::screen_record_jobs::Status, BridgeError> {
    recordings.begin(id).map_err(error)
}
#[tauri::command]
async fn screen_record_start(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    id: String,
) -> Result<(), BridgeError> {
    #[cfg(feature = "native-smoke")]
    if std::env::var("LCONNECT_NATIVE_RECORDING").as_deref() == Ok("1") {
        return recordings.start_synthetic(&id).map_err(error);
    }
    recordings.start(&id).map_err(error)
}
#[tauri::command]
fn screen_record_status(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
) -> Result<Option<lconnect_gui::screen_record_jobs::Status>, BridgeError> {
    recordings.status().map_err(error)
}
#[tauri::command]
async fn screen_record_snapshot(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    id: String,
) -> Result<tauri::ipc::Response, BridgeError> {
    recordings
        .snapshot(&id)
        .await
        .map(tauri::ipc::Response::new)
        .map_err(error)
}
#[tauri::command]
async fn screen_record_control(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    id: String,
    action: lconnect_gui::screen_record_jobs::Action,
) -> Result<(), BridgeError> {
    recordings.command(&id, action).await.map_err(error)
}
#[tauri::command]
fn screen_record_admit(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    imports: tauri::State<'_, MediaImports>,
    recording_id: String,
    id: String,
    name: String,
    fit: Fit,
    cut: Option<lconnect_gui::media_prepare::VideoCut>,
) -> Result<MediaStatus, BridgeError> {
    imports
        .begin_recording(
            &recordings,
            &recording_id,
            id,
            name,
            lconnect_gui::media_import::EditOptions {
                range: None,
                fit,
                cut,
                adjustments: lconnect_gui::video_adjustments::VideoAdjustments::default(),
            },
        )
        .map_err(error)
}
#[tauri::command]
async fn screen_record_prepare(
    imports: tauri::State<'_, MediaImports>,
    id: String,
) -> Result<(), BridgeError> {
    let imports = imports.inner().clone();
    tauri::async_runtime::spawn(async move { imports.prepare_review(&id).await.map_err(error) })
        .await
        .map_err(error)?
}

#[tauri::command]
async fn screen_record_cancel(
    recordings: tauri::State<'_, lconnect_gui::screen_record_jobs::Recordings>,
    id: String,
) -> Result<(), BridgeError> {
    recordings.cancel(&id).await.map_err(error)
}

#[tauri::command]
async fn begin_screenshot(
    capture: tauri::State<'_, lconnect_gui::screenshot::Screenshots>,
) -> Result<String, BridgeError> {
    capture.begin().await.map_err(error)
}
#[tauri::command]
async fn cancel_screenshot(
    id: String,
    capture: tauri::State<'_, lconnect_gui::screenshot::Screenshots>,
) -> Result<lconnect_gui::screenshot::CancelOutcome, BridgeError> {
    capture.cancel(&id).await.map_err(error)
}

#[tauri::command]
async fn pending_screenshot_cleanup(
    capture: tauri::State<'_, lconnect_gui::screenshot::Screenshots>,
) -> Result<Option<String>, BridgeError> {
    Ok(capture.pending_cleanup().await)
}

#[tauri::command]
async fn capture_screenshot(
    id: String,
    capture: tauri::State<'_, lconnect_gui::screenshot::Screenshots>,
) -> Result<tauri::ipc::Response, BridgeError> {
    capture
        .capture(&id)
        .await
        .map(tauri::ipc::Response::new)
        .map_err(error)
}

#[tauri::command]
async fn image_editor(
    editor: tauri::State<'_, lconnect_gui::image_editor::Editor>,
    bytes: Vec<u8>,
    edit: Option<lconnect_gui::image_editor::Edit>,
) -> Result<lconnect_gui::image_editor::Prepared, BridgeError> {
    editor.prepare(bytes, edit).await.map_err(error)
}

#[tauri::command]
async fn import_image(
    bridge: tauri::State<'_, Bridge>,
    bytes: Vec<u8>,
    name: Option<String>,
) -> Result<String, BridgeError> {
    if bytes.len() > lconnect_image::assets::MAX_ASSET_BYTES {
        return Err(error("Image exceeds 16 MiB"));
    }
    let normalized =
        tokio::task::spawn_blocking(move || lconnect_image::assets::normalize_asset(&bytes))
            .await
            .map_err(error)?
            .map_err(error)?;
    if normalized.len() > 1024 * 1024 {
        return Err(error(
            "Normalized PNG exceeds 1 MiB; reduce image detail or dimensions",
        ));
    }
    let result = daemon_call(
        bridge,
        "import_theme_asset".into(),
        serde_json::json!({"bytes":normalized,"name":name}),
    )
    .await?;
    serde_json::from_value(result).map_err(error)
}

#[tauri::command]
fn cooling_preset(
    target: lconnect_ipc::CoolingTarget,
    preset: String,
    sensor_id: String,
) -> Result<lconnect_ipc::CoolingPolicy, BridgeError> {
    let name = Value::String(preset);
    let policy = match target {
        lconnect_ipc::CoolingTarget::Fan => serde_json::from_value::<lconnect_ipc::FanPreset>(name)
            .map_err(error)?
            .policy(sensor_id, 2.0, 100),
        lconnect_ipc::CoolingTarget::Pump => {
            serde_json::from_value::<lconnect_ipc::PumpPreset>(name)
                .map_err(error)?
                .policy(sensor_id, 2.0, 100)
        }
    };
    policy.validate(target).map_err(error)?;
    Ok(policy)
}
#[tauri::command]
fn cooling_mode_default(
    target: lconnect_ipc::CoolingTarget,
    id: String,
    sensor_id: String,
) -> Result<lconnect_ipc::CoolingModeProfile, BridgeError> {
    lconnect_ipc::lcdv2_default_cooling_mode(target, &id, &sensor_id).map_err(error)
}
#[derive(Default)]
struct Closing {
    requested: std::sync::atomic::AtomicBool,
    completed: std::sync::atomic::AtomicBool,
}
fn close_after_media(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::Manager;
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if window
            .state::<Closing>()
            .requested
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return;
        }
        if window
            .state::<Arc<lconnect_gui::desktop_runtime::Desktop>>()
            .hide_on_close()
            && window.hide().is_ok()
        {
            return;
        }
        request_exit(window.app_handle());
    }
}
fn request_exit(app: &tauri::AppHandle) {
    use tauri::Manager;
    if app
        .state::<Closing>()
        .requested
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return;
    }
    #[cfg(feature = "native-smoke")]
    eprintln!("native close: request admitted");
    let imports = app.state::<MediaImports>().inner().clone();
    let thumbnails = app.state::<Thumbnails>().inner().clone();
    let sources = app.state::<MediaSources>().inner().clone();
    let http = app
        .state::<lconnect_gui::media_http::MediaHttp>()
        .inner()
        .clone();
    let capture = app
        .state::<lconnect_gui::screenshot::Screenshots>()
        .inner()
        .clone();
    let groups = app
        .state::<lconnect_gui::group_jobs::Groups>()
        .inner()
        .clone();
    let archives = app
        .state::<lconnect_gui::archive_jobs::Archives>()
        .inner()
        .clone();
    let recordings = app
        .state::<lconnect_gui::screen_record_jobs::Recordings>()
        .inner()
        .clone();
    let app = app.clone();
    let desktop = app
        .state::<Arc<lconnect_gui::desktop_runtime::Desktop>>()
        .inner()
        .clone();
    tauri::async_runtime::spawn(async move {
        let source_shutdown = async move {
            loop {
                let owner = sources.clone();
                if tokio::task::spawn_blocking(move || owner.shutdown()).await?? {
                    return Ok::<(), anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        let (imports, thumbnails, sources, http, capture, recordings, archives, groups, desktop) = tokio::join!(
            imports.shutdown(),
            thumbnails.shutdown(),
            source_shutdown,
            http.shutdown(),
            capture.shutdown(),
            recordings.shutdown(),
            archives.shutdown(),
            async {
                let result = groups.shutdown().await;
                #[cfg(feature = "native-smoke")]
                eprintln!("native close: group worker joined ({})", result.is_ok());
                result
            },
            desktop.shutdown()
        );
        #[cfg(feature = "native-smoke")]
        eprintln!("native close: all workers joined");
        match imports
            .and(thumbnails)
            .and(sources)
            .and(http)
            .and(capture)
            .and(recordings)
            .and(archives)
            .and(groups)
            .and(desktop)
        {
            Ok(()) => {
                app.state::<Closing>()
                    .completed
                    .store(true, std::sync::atomic::Ordering::Release);
                app.exit(0);
            }
            Err(error) => {
                eprintln!("Cannot finish media shutdown: {error}");
                app.state::<Closing>()
                    .requested
                    .store(false, std::sync::atomic::Ordering::Release);
            }
        }
    });
}
fn desktop_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    use tauri::Manager;
    if let tauri::RunEvent::ExitRequested { api, .. } = event {
        if !app
            .state::<Closing>()
            .completed
            .load(std::sync::atomic::Ordering::Acquire)
        {
            api.prevent_exit();
            request_exit(app);
        }
    }
}
#[tauri::command]
async fn desktop_settings_get(
    state: tauri::State<'_, Arc<lconnect_gui::desktop_runtime::Desktop>>,
) -> Result<lconnect_gui::desktop_runtime::Snapshot, BridgeError> {
    state.inner().clone().read().await.map_err(error)
}
#[tauri::command]
async fn desktop_settings_save(
    state: tauri::State<'_, Arc<lconnect_gui::desktop_runtime::Desktop>>,
    expected_revision: u64,
    preferences: lconnect_gui::desktop_settings::Preferences,
) -> Result<lconnect_gui::desktop_runtime::Snapshot, BridgeError> {
    state
        .inner()
        .clone()
        .save(expected_revision, preferences)
        .await
        .map_err(error)
}
#[tauri::command]
fn desktop_set_language(
    state: tauri::State<'_, Arc<lconnect_gui::desktop_runtime::Desktop>>,
    language: lconnect_ipc::InterfaceLanguage,
) -> Result<(), BridgeError> {
    state.set_language(language).map_err(error)
}

#[tauri::command]
fn desktop_exit(app: tauri::AppHandle) {
    request_exit(&app);
}

fn desktop_builder() -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default();
    // A broken/missing session bus must not prevent the settings window from
    // opening. The plugin's Linux constructor assumes a valid session address.
    if std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .ok()
        .is_some_and(|s| zbus::Address::try_from(s.as_str()).is_ok())
    {
        builder.plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--autostart") {
                lconnect_gui::desktop_runtime::show(app);
            }
        }))
    } else {
        builder
    }
}

#[cfg(not(feature = "native-smoke"))]
fn main() {
    if lconnect_gui::screen_record_process::dispatch_worker() {
        return;
    }
    if let Err(error) = lconnect_gui::desktop_startup::configure_graphics() {
        eprintln!("Cannot initialize graphics compatibility: {error}");
        std::process::exit(1);
    }
    with_media_preview(desktop_builder())
        .manage(lconnect_gui::group_jobs::Groups::default())
        .manage(lconnect_gui::archive_jobs::Archives::default())
        .manage(Bridge::default())
        .manage(lconnect_gui::screenshot::Screenshots::default())
        .manage(lconnect_gui::screen_record_jobs::Recordings::default())
        .manage(lconnect_gui::image_editor::Editor::default())
        .manage(MediaImports::default())
        .manage(MediaSources::default())
        .manage(lconnect_gui::media_http::MediaHttp::default())
        .manage(Thumbnails::default())
        .manage(Closing::default())
        .on_window_event(close_after_media)
        .setup(|app| {
            lconnect_gui::desktop_runtime::setup(app, request_exit)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_settings_get,
            desktop_set_language,
            desktop_settings_save,
            desktop_exit,
            group_archive_start,
            group_windows_prepare,
            group_archive_status,
            group_archive_action,
            daemon_call,
            profile_archive_start,
            profile_archive_status,
            profile_archive_cancel,
            profile_archive_recover,
            profile_archive_dismiss,
            cooling_preset,
            cooling_mode_default,
            import_image,
            image_editor,
            capture_screenshot,
            begin_screenshot,
            cancel_screenshot,
            pending_screenshot_cleanup,
            screen_record_begin,
            screen_record_start,
            screen_record_status,
            screen_record_snapshot,
            screen_record_control,
            screen_record_cancel,
            screen_record_prepare,
            screen_record_admit,
            media_playback_url,
            media_begin,
            media_append,
            media_inspect,
            media_prepare_inspected,
            media_start,
            media_source_prepare,
            media_source_start,
            media_source_status,
            media_source_cancel,
            media_confirm,
            media_status,
            media_cancel,
            media_thumbnail,
            media_thumbnail_cancel
        ])
        .build(tauri::generate_context!())
        .expect("desktop runtime failed")
        .run(desktop_event);
}
#[cfg(test)]
mod tests {
    #[test]
    fn archive_bridge_keeps_typed_notice_separate_from_raw_diagnostic_and_rpc_code() {
        use lconnect_gui::archive_messages::ArchiveNotice;
        let error =
            anyhow::Error::from(ArchiveNotice::DestinationExists).context("Настройки {1}.lcp");
        let value = serde_json::to_value(super::archive_error(error)).unwrap();
        assert_eq!(value["message_code"], "destination_exists");
        assert_eq!(value["message"], "Настройки {1}.lcp");
        assert!(value.get("code").is_none());
        let plain =
            serde_json::to_value(super::archive_error(anyhow::anyhow!("Настройки"))).unwrap();
        assert!(plain.get("message_code").is_none());
    }
    #[test]
    fn preset_bridge_uses_typed_reference_and_rejects_wrong_target() {
        use lconnect_ipc::{CoolingPolicy, CoolingTarget};
        let policy =
            super::cooling_preset(CoolingTarget::Pump, "quiet".into(), "cpu:test".into()).unwrap();
        assert!(matches!(policy, CoolingPolicy::CurveRpm { .. }));
        assert!(
            super::cooling_preset(CoolingTarget::Fan, "performance".into(), "cpu:test".into())
                .is_err()
        );
        assert!(super::cooling_preset(CoolingTarget::Fan, "high".into(), "".into()).is_err());
        let mode =
            super::cooling_mode_default(CoolingTarget::Pump, "FixRPM".into(), "".into()).unwrap();
        assert_eq!(mode.policy, CoolingPolicy::FixedRpm { rpm: 2200 });
        assert!(mode.use_default_name);
        assert!(super::cooling_mode_default(
            CoolingTarget::Fan,
            "FixRPM".into(),
            "cpu:test".into()
        )
        .is_err());
        assert!(
            super::cooling_mode_default(CoolingTarget::Pump, "PWMQuiet".into(), "".into()).is_err()
        );
    }
    #[test]
    fn maintenance_and_raw_commands_are_not_exposed() {
        for method in [
            "raw_send",
            "reset_device",
            "request_reset_token",
            "update_firmware",
            "begin_firmware_update",
            "clear_firmware_recovery",
            "firmware_reconnect_probe",
            "shell",
        ] {
            assert!(!super::allowed(method));
        }
        assert!(super::allowed("get_firmware"));
        assert!(super::allowed("get_firmware_recovery"));
        assert!(super::allowed("preview_lcd_theme"));
        assert!(super::allowed("font_upload"));
        for method in [
            "list_prepared_clips",
            "delete_prepared_clip",
            "get_lcd_media",
            "set_lcd_media",
        ] {
            assert!(super::allowed(method));
        }
        for method in [
            "load_prepared_clip",
            "media_upload",
            "load_media_first_frame",
            "play_lcd_clip",
            "import_prepared_clip",
        ] {
            assert!(!super::allowed(method));
        }
    }
}

#[cfg(feature = "native-smoke")]
#[tauri::command]
fn native_smoke_result(
    app: tauri::AppHandle,
    result: Value,
    keep_open: Option<bool>,
) -> Result<(), String> {
    let root =
        std::path::PathBuf::from(std::env::var("LCONNECT_SMOKE_ROOT").map_err(|e| e.to_string())?);
    if !root.is_absolute() || !root.join("fixture-marker").is_file() {
        return Err("fixture root required".into());
    }
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if keep_open != Some(true) {
        app.exit(if result["ok"] == true { 0 } else { 1 });
    }
    Ok(())
}

#[cfg(feature = "native-smoke")]
#[tauri::command]
async fn native_media_step(sequence: u32, action: String) -> Result<(), String> {
    if sequence == 0
        || !matches!(
            action.as_str(),
            "chooser_cancel"
                | "chooser_bad"
                | "chooser_editor"
                | "editor_saved"
                | "editor_reedited"
                | "editor_recorded"
                | "screenshot_cancel_ready"
                | "screenshot_close_ready"
                | "chooser_mp4"
                | "chooser_gif"
                | "chooser_slow"
                | "cancel_worker"
                | "restart_daemon"
                | "seed_long_v2"
                | "long_loop"
                | "close_worker"
                | "recording_stall"
                | "recording_crash"
                | "recording_cancel"
                | "recording_close"
                | "recording_reaped"
                | "recording_recovered"
                | "recovery_ready"
                | "recovery_allow"
                | "owner_loss_new_ready"
        )
    {
        return Err("Unsupported media fixture step".into());
    }
    tokio::task::spawn_blocking(move || {
        let root = std::path::PathBuf::from(
            std::env::var("LCONNECT_SMOKE_ROOT").map_err(|e| e.to_string())?,
        );
        if !root.is_absolute()
            || !root.join("fixture-marker").is_file()
            || std::env::var("LCONNECT_NATIVE_MEDIA").as_deref() != Ok("1")
        {
            return Err("Isolated media fixture required".into());
        }
        let bytes = serde_json::to_vec(&serde_json::json!({"sequence":sequence,"action":action}))
            .map_err(|e| e.to_string())?;
        std::fs::write(root.join("media-step.tmp"), bytes).map_err(|e| e.to_string())?;
        std::fs::rename(root.join("media-step.tmp"), root.join("media-step.json"))
            .map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now()
            + Duration::from_secs(if action == "long_loop" { 45 } else { 25 });
        loop {
            if std::fs::read_to_string(root.join("media-ack"))
                .ok()
                .is_some_and(|v| v == sequence.to_string())
            {
                return Ok(());
            }
            if std::time::Instant::now() > deadline {
                return Err("Media fixture step timed out".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(feature = "native-smoke")]
#[tauri::command]
fn native_keyboard_step(sequence: u32, key: String, probe: Option<String>) -> Result<(), String> {
    if probe.as_deref().is_some_and(|page| {
        !matches!(
            page,
            "settings"
                | "cooling"
                | "lighting"
                | "sensors"
                | "display"
                | "reset"
                | "fixture_reset_fault"
                | "fixture_reset_recover"
                | "fixture_disconnect"
                | "fixture_reconnect"
                | "fixture_events_ready"
                | "fixture_file_cancel"
                | "fixture_file_select"
                | "fixture_file_font_cancel"
                | "fixture_file_font_select"
                | "fixture_file_font_invalid"
                | "fixture_file_font_large"
                | "image_crop_x"
                | "image_crop_y"
                | "image_crop_side"
                | "image_crop_pointer"
                | "capture_region_pointer"
                | "video_region_pointer"
                | "canvas_move"
                | "canvas_resize"
                | "canvas_cancel"
                | "canvas_no_writes"
        )
    }) {
        return Err("unsupported fixture probe".into());
    }
    if !matches!(
        key.as_str(),
        "Tab"
            | "shift+Tab"
            | "Return"
            | "space"
            | "Down"
            | "Up"
            | "Left"
            | "Right"
            | "End"
            | "Home"
            | "Escape"
            | "ctrl+Right"
            | "shift+Right"
    ) {
        return Err("unsupported fixture key".into());
    }
    let root =
        std::path::PathBuf::from(std::env::var("LCONNECT_SMOKE_ROOT").map_err(|e| e.to_string())?);
    if !root.is_absolute() || !root.join("fixture-marker").is_file() {
        return Err("fixture root required".into());
    }
    let bytes =
        serde_json::to_vec(&serde_json::json!({"sequence": sequence, "key": key, "probe": probe}))
            .map_err(|e| e.to_string())?;
    std::fs::write(root.join("keyboard-step.tmp"), bytes).map_err(|e| e.to_string())?;
    std::fs::rename(
        root.join("keyboard-step.tmp"),
        root.join("keyboard-step.json"),
    )
    .map_err(|e| e.to_string())
}

#[cfg(feature = "native-smoke")]
#[tauri::command]
async fn native_desktop_step(sequence: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let root = std::path::PathBuf::from(
            std::env::var("LCONNECT_SMOKE_ROOT").map_err(|e| e.to_string())?,
        );
        if sequence == 0
            || !root.is_absolute()
            || !root.join("fixture-marker").is_file()
            || std::env::var("LCONNECT_NATIVE_DESKTOP").as_deref() != Ok("1")
        {
            return Err("Isolated desktop fixture required".into());
        }
        std::fs::write(root.join("desktop-step"), sequence.to_string())
            .map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + Duration::from_secs(25);
        while std::time::Instant::now() < deadline {
            if std::fs::read_to_string(root.join("desktop-ack"))
                .ok()
                .as_deref()
                == Some(sequence.to_string().as_str())
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err("Desktop harness acknowledgement timed out".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(feature = "native-smoke")]
fn main() {
    if lconnect_gui::screen_record_process::dispatch_worker() {
        return;
    }
    use tauri::Manager;
    let root = std::path::PathBuf::from(
        std::env::var("LCONNECT_SMOKE_ROOT").expect("fixture root required"),
    );
    assert!(root.is_absolute() && root.join("fixture-marker").is_file());
    assert_eq!(
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        Some(root.as_os_str())
    );
    with_media_preview(desktop_builder()).manage(lconnect_gui::group_jobs::Groups::default()).manage(lconnect_gui::archive_jobs::Archives::default()).manage(Bridge::default()).manage(lconnect_gui::screenshot::Screenshots::default()).manage(lconnect_gui::screen_record_jobs::Recordings::default()).manage(lconnect_gui::image_editor::Editor::default()).manage(MediaImports::default()).manage(MediaSources::default()).manage(lconnect_gui::media_http::MediaHttp::default()).manage(Thumbnails::default()).manage(Closing::default()).on_window_event(close_after_media)
        .invoke_handler(tauri::generate_handler![desktop_set_language,desktop_settings_get,desktop_settings_save,desktop_exit,group_archive_start,group_windows_prepare,group_archive_status,group_archive_action,daemon_call,profile_archive_start,profile_archive_status,profile_archive_cancel,profile_archive_recover,profile_archive_dismiss,cooling_preset,cooling_mode_default,import_image,image_editor,capture_screenshot,begin_screenshot,cancel_screenshot,pending_screenshot_cleanup,screen_record_begin,screen_record_start,screen_record_status,screen_record_snapshot,screen_record_control,screen_record_cancel,screen_record_prepare,screen_record_admit,media_playback_url,media_begin,media_append,media_inspect,media_prepare_inspected,media_start,media_source_prepare,media_source_start,media_source_status,media_source_cancel,media_confirm,media_status,media_cancel,media_thumbnail,media_thumbnail_cancel,native_smoke_result,native_keyboard_step,native_media_step,native_desktop_step])
        .setup(|app| {
            lconnect_gui::desktop_runtime::setup(app, request_exit)?;
            app.add_capability(r#"{"identifier":"smoke","windows":["main"],"permissions":["allow-native-smoke-result","allow-native-keyboard-step","allow-native-media-step","allow-native-desktop-step"]}"#)?;
            Ok(())
        })
        .on_page_load(|webview,payload| {
            if matches!(payload.event(),tauri::webview::PageLoadEvent::Finished) {
                let dialogs = std::env::var("LCONNECT_NATIVE_DIALOGS").as_deref() == Ok("1");
                webview.eval(format!("window.__nativeDialogs={dialogs};"))
                    .expect("expose fixture dialog mode");
                let zoom = match std::env::var("LCONNECT_NATIVE_ZOOM").as_deref() {
                    Ok("2") => 2.0,
                    Ok("4") => 4.0,
                    Ok("1") | Err(_) => 1.0,
                    Ok(_) => panic!("fixture zoom must be 1, 2 or 4"),
                };
                webview.set_zoom(zoom).expect("set fixture webview zoom");
                webview.eval(format!("window.__nativeZoom={zoom};"))
                    .expect("expose fixture zoom");
                // Fixture bytes are embedded only in the isolated native-smoke build.
                let bytes = include_bytes!("../../../crates/lconnect-image/assets/DejaVuSans.ttf");
                if let Ok(json) = serde_json::to_string(bytes.as_slice()) {
                    let _ = webview.eval(format!("window.__nativeFontBytes={json};"));
                }
                let group_kind = if std::env::var("LCONNECT_NATIVE_GROUP_KIND").as_deref()==Ok("fan_and_lighting") {"fan_and_lighting"} else {"screen"};
                webview.eval(format!("window.__nativeGroupKind={group_kind:?};")).expect("expose fixture group");
                let recover_windows=std::env::var("LCONNECT_NATIVE_WINDOWS_RECOVER").as_deref()==Ok("1");
                webview.eval(format!("window.__nativeWindowsRecover={recover_windows};")).expect("Windows fixture phase");
                let recover_reset = std::env::var("LCONNECT_NATIVE_CONTROL_RESET_RECOVER").as_deref()==Ok("1");
                webview.eval(format!("window.__nativeResetRecover={recover_reset};")).expect("expose reset fixture phase");
                let desktop_phase=std::env::var("LCONNECT_NATIVE_DESKTOP_PHASE").unwrap_or_else(|_| "1".into());
                webview.eval(format!("window.__nativeDesktopPhase={desktop_phase:?};")).expect("desktop fixture phase");
                let desktop_language=if std::env::var("LCONNECT_NATIVE_DESKTOP_LANGUAGE").as_deref()==Ok("en-US") {"en-US"} else {"ru-RU"};
                webview.eval(format!("window.__nativeDesktopLanguage={desktop_language:?};")).expect("desktop fixture language");
                let fps_log=std::env::var("LCONNECT_NATIVE_FPS_LOG").unwrap_or_default();
                webview.eval(format!("window.__nativeFpsLog={};",serde_json::to_string(&fps_log).expect("encode private FPS log"))).expect("expose private FPS log");
                let script = if std::env::var("LCONNECT_NATIVE_RECOVERY").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-firmware-recovery.js")
                } else if std::env::var("LCONNECT_NATIVE_FPS").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-fps-settings.js")
                } else if std::env::var("LCONNECT_NATIVE_FIRMWARE").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-firmware-preflight.js")
                } else if std::env::var("LCONNECT_NATIVE_CANVAS").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-canvas.js")
                } else if std::env::var("LCONNECT_NATIVE_IMAGE_EDITOR").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-image-editor.js")
                } else if std::env::var("LCONNECT_NATIVE_LANGUAGE").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-interface-language.js")
                } else if std::env::var("LCONNECT_NATIVE_UNITS").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-display-settings.js")
                } else if std::env::var("LCONNECT_NATIVE_SPEC").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-system-spec.js")
                } else if std::env::var("LCONNECT_NATIVE_DESKTOP").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-desktop.js")
                } else if std::env::var("LCONNECT_NATIVE_BASIC_EDITORS").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-basic-editors.js")
                } else if std::env::var("LCONNECT_NATIVE_WINDOWS_SCREEN").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-windows-screen.js")
                } else if std::env::var("LCONNECT_NATIVE_WINDOWS_CONTROL").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-windows-control.js")
                } else if std::env::var("LCONNECT_NATIVE_CONTROL_RESET").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-control-reset.js")
                } else if std::env::var("LCONNECT_NATIVE_GROUP_ARCHIVE").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-group-archive.js")
                } else if std::env::var("LCONNECT_NATIVE_ARCHIVE").as_deref()==Ok("1") {
                    include_str!("../../scripts/native-archive.js")
                } else if std::env::var("LCONNECT_NATIVE_LIGHTING_PALETTE").as_deref() == Ok("1") {
                    include_str!("../../scripts/native-lighting-palette.js")
                } else if std::env::var("LCONNECT_NATIVE_WEATHER").as_deref() == Ok("1") {
                    include_str!("../../scripts/native-weather.js")
                } else if std::env::var("LCONNECT_NATIVE_RECORDING").as_deref() == Ok("1") {
                    if std::env::var("LCONNECT_NATIVE_RECORDING_FAULTS").as_deref() == Ok("1") {
                        include_str!("../../scripts/native-recording-faults.js")
                    } else if std::env::var("LCONNECT_NATIVE_RECORDING_LIMIT").as_deref() == Ok("1") {
                        include_str!("../../scripts/native-recording-limit.js")
                    } else {
                        include_str!("../../scripts/native-recording.js")
                    }
                } else if std::env::var("LCONNECT_NATIVE_SCREENSHOT").as_deref() == Ok("1") {
                    include_str!("../../scripts/native-screenshot.js")
                } else if std::env::var("LCONNECT_NATIVE_MEDIA").as_deref() == Ok("1") {
                    include_str!("../../scripts/native-media.js")
                } else if std::env::var("LCONNECT_NATIVE_KEYBOARD").as_deref() == Ok("1") {
                    include_str!("../../scripts/native-keyboard.js")
                } else {
                    include_str!("../../scripts/native-smoke.js")
                };
                let cooling_defaults=std::env::var("LCONNECT_NATIVE_COOLING_DEFAULTS").as_deref()==Ok("1");
                let _=webview.eval(format!("window.__nativeCoolingDefaults={cooling_defaults};"));
                let _=webview.eval(script);
            }
        })
        .build(tauri::generate_context!()).expect("native smoke runtime").run(desktop_event);
}
