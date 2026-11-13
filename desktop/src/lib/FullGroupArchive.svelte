<script lang="ts">
  import { t } from './i18n.svelte';
  import ExperimentalNotice from './ExperimentalNotice.svelte';
  import WindowsScreenPreview from './WindowsScreenPreview.svelte';
  import type { DisplayGeometry } from './device-capabilities';
  import WindowsScreenBindings from './WindowsScreenBindings.svelte';
  import type { ScreenOptions } from './windows-screen';
  import type { FontChoice } from './font-upload';
  import WindowsLightingReview from './WindowsLightingReview.svelte';
  import {
    windowsRoles,
    windowsBindingInvalid,
    windowsSourceUnverified,
    windowsUnit,
    windowsUnitLabel,
    windowsRoleLabel,
    windowsOptions,
    windowsOptionsKey,
    emptyWindowsIds,
    type WindowsIds,
  } from './windows-control';
  import {
    ArchiveUiError,
    archiveError,
    archiveMessage,
    archiveDiagnostic,
    type ArchiveMessage,
  } from './archive-messages';
  import { presentation } from './presentation.svelte';
  import { onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { coolingModeLabel } from './cooling-modes';
  import { type Sensor } from './api';
  import {
    policySummary,
    groupNames,
    groupPhases,
    groupIdle,
    groupPendingKey,
    readGroupPending,
    type FullGroup,
    type GroupTarget,
    type GroupMode,
    type GroupStatus,
  } from './group-archive';
  let {
    device,
    connected,
    sensors = [],
    fonts = [],
    geometry = null,
    disabled = false,
    onbusy,
    onchange,
  }: {
    device: string;
    connected: boolean;
    sensors?: Sensor[];
    fonts?: FontChoice[];
    geometry?: DisplayGeometry | null;
    disabled?: boolean;
    onbusy: (value: boolean) => void;
    onchange: (dev: string) => void;
  } = $props();
  const initial = readGroupPending();
  let pending = $state<GroupTarget | null>(initial),
    job = $state<GroupStatus | null>(null);
  let group = $state<FullGroup>(initial?.group ?? 'screen'),
    recoveryId = $state(initial?.id ?? '');
  let recoveryDev = $state(initial?.dev ?? ''),
    recoveryMode = $state<GroupMode>(initial?.mode ?? 'import');
  let resetSensor = $state('');
  let windowsRecord = $state(''),
    windowsIds = $state<WindowsIds>(emptyWindowsIds());
  let windowsFormFor = '';
  const windowsDraft = $derived(
    windowsOptions(
      windowsRecord,
      windowsIds,
      sensors,
      job?.windows_source?.controllers.map((c) => c.record_index) ?? [],
    ),
  );
  const windowsCurrent = $derived(
    !!windowsDraft && windowsOptionsKey(windowsDraft) === windowsOptionsKey(job?.windows_options),
  );
  const temperatureSensors = $derived(sensors.filter((s) => s.unit === 'celsius'));
  let open = $state(false),
    requesting = $state(false),
    screenPreviewBusy = $state(false);
  let error = $state<string | ArchiveMessage>('');
  let mutation = 0,
    handled = '';
  const active = $derived(requesting || (!!job && !groupIdle(job.phase)));
  const windowsEditable = $derived(
    !!job &&
      job.mode === 'windows' &&
      job.dev === device &&
      connected &&
      !disabled &&
      !active &&
      ['bindings', 'prepared', 'failed'].includes(job.phase),
  );
  const blocked = $derived(active || disabled || !connected || !device || !!pending);
  const target = $derived<GroupTarget>({
    id: recoveryId,
    dev: recoveryDev || device,
    group,
    mode: recoveryMode,
  });
  const recoveryValid = $derived(
    /^[a-f0-9]{32}$/.test(target.id) &&
      !!target.dev &&
      (!['windows', 'reset'].includes(target.mode) || target.group === 'fan_and_lighting') &&
      (target.mode !== 'windows_screen' || target.group === 'screen'),
  );
  const applicable = $derived(
    !!job && job.dev === device && job.group === group && connected && !disabled && !active,
  );
  const cancellable = $derived(
    !!job &&
      (!['reset', 'windows'].includes(job.mode) ||
        ['bindings', 'prepared', 'inspecting'].includes(job.phase)) &&
      !['applying', 'recovering', 'cancelling', 'complete', 'cancelled', 'failed'].includes(
        job.phase,
      ) &&
      !requesting,
  );
  $effect(() => {
    onbusy(active || screenPreviewBusy);
  });
  onDestroy(() => onbusy(false));
  function remember(value: GroupTarget) {
    const current = readGroupPending();
    if (
      current &&
      (current.id !== value.id ||
        current.dev !== value.dev ||
        current.group !== value.group ||
        current.mode !== value.mode)
    )
      throw new ArchiveUiError('pending_group_operation');
    localStorage.setItem(groupPendingKey, JSON.stringify(value));
    pending = value;
    recoveryId = value.id;
    recoveryDev = value.dev;
    group = value.group;
    recoveryMode = value.mode;
  }
  function accept(status: GroupStatus | null) {
    if (!status) return;
    if (
      pending &&
      (status.id !== pending.id ||
        status.dev !== pending.dev ||
        status.group !== pending.group ||
        status.mode !== pending.mode)
    ) {
      error = 'Another group operation exists in the application. Finish recovering it first.';
      return;
    }
    if (!pending)
      remember({ id: status.id, dev: status.dev, group: status.group, mode: status.mode });
    if (status.mode === 'windows' && windowsFormFor !== status.id) {
      windowsFormFor = status.id;
      windowsRecord = status.windows_options ? String(status.windows_options.record) : '';
      windowsIds = emptyWindowsIds();
      if (status.windows_options)
        for (const role of windowsRoles)
          windowsIds[role] = status.windows_options.bindings[role].sensor_id;
    }
    job = status;
    if (status.readback) {
      const key = JSON.stringify([status.id, status.readback.receipt, status.readback.runtime]);
      if (key !== handled) {
        handled = key;
        onchange(status.dev);
      }
    }
  }
  $effect(() => {
    if (!connected || !(open || active)) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      const epoch = mutation;
      try {
        const result = await invoke<GroupStatus | null>('group_archive_status');
        if (!stopped && epoch === mutation) accept(result);
      } catch (e) {
        if (!stopped && epoch === mutation) error = archiveError(e);
      }
      if (!stopped) timer = setTimeout(poll, 500);
    }
    timer = setTimeout(poll, 0);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });
  async function start(mode: GroupMode) {
    if (
      blocked ||
      (mode === 'reset' && (group !== 'fan_and_lighting' || !resetSensor)) ||
      (mode === 'windows' && group !== 'fan_and_lighting') ||
      (mode === 'windows_screen' && group !== 'screen')
    )
      return;
    const request: GroupTarget = {
      id: crypto.randomUUID().replaceAll('-', ''),
      dev: device,
      group,
      mode,
    };
    mutation++;
    requesting = true;
    error = '';
    try {
      const current = await invoke<GroupStatus | null>('group_archive_status');
      if (
        current &&
        (!groupIdle(current.phase) || current.phase === 'prepared' || current.phase === 'uncertain')
      ) {
        accept(current);
        return;
      }
      remember(request);
      accept(
        await invoke<GroupStatus>('group_archive_start', {
          language: presentation.language,
          request: mode === 'reset' ? { ...request, sensor_id: resetSensor } : request,
        }),
      );
    } catch (e) {
      error = archiveError(e);
    } finally {
      requesting = false;
      mutation++;
    }
  }
  async function prepareWindows() {
    if (!windowsEditable || !windowsDraft || !job) return;
    const request: GroupTarget = { id: job.id, dev: job.dev, group: job.group, mode: 'windows' };
    mutation++;
    requesting = true;
    error = '';
    try {
      accept(
        await invoke<GroupStatus>('group_windows_prepare', { request, options: windowsDraft }),
      );
    } catch (e) {
      const diagnostic = archiveError(e);
      error = diagnostic.message_code
        ? diagnostic
        : { ...diagnostic, message_code: 'windows_preparation_failed' };
    } finally {
      requesting = false;
      mutation++;
    }
  }
  async function prepareScreen(options: ScreenOptions | { record: number; inspect_only: true }) {
    if (!job || job.mode !== 'windows_screen' || job.phase !== 'bindings' || !applicable) return;
    const request: GroupTarget = { id: job.id, dev: job.dev, group: job.group, mode: job.mode };
    mutation++;
    requesting = true;
    error = '';
    try {
      accept(await invoke<GroupStatus>('group_windows_prepare', { request, options }));
    } catch (e) {
      error = archiveError(e);
    } finally {
      requesting = false;
      mutation++;
    }
  }
  async function act(action: 'finish' | 'recover' | 'cancel' | 'reapply' | 'dismiss') {
    if (
      requesting ||
      (!connected && action !== 'dismiss' && !(action === 'cancel' && job?.mode === 'windows')) ||
      !recoveryValid
    )
      return;
    if (['finish', 'reapply'].includes(action) && (!applicable || screenPreviewBusy)) return;
    if (
      action === 'finish' &&
      job?.mode === 'windows' &&
      job.phase === 'prepared' &&
      !windowsCurrent
    )
      return;
    const request: GroupTarget = { ...target };
    mutation++;
    requesting = true;
    error = '';
    try {
      remember(request);
      const status = await invoke<GroupStatus | null>('group_archive_action', {
        request,
        action,
        expected: action === 'reapply' ? (job?.readback?.runtime.revision ?? null) : null,
      });
      if (action === 'dismiss') {
        if (readGroupPending()?.id === request.id) localStorage.removeItem(groupPendingKey);
        pending = readGroupPending();
        job = null;
        recoveryId = '';
        recoveryDev = '';
        handled = '';
      } else accept(status);
    } catch (e) {
      error = archiveError(e);
    } finally {
      requesting = false;
      mutation++;
    }
  }
</script>

<details ontoggle={(e) => (open = e.currentTarget.open)}>
  <summary>{t('Settings group backup')}</summary>
  <p>
    {t(
      'An LL-Control archive (.lcg) saves the selected group and its slots. Screen includes images, videos and fonts. Windows ZIP 2.1.28 can be imported separately for Screen and FanAndLighting.',
    )}
  </p>
  <label
    >{t('Settings group')}<select
      aria-label={t('Settings group')}
      bind:value={group}
      disabled={active || !!pending}
      ><option value="screen">{t('Screen')}</option><option value="fan_and_lighting"
        >{t('Fans and lighting')}</option
      ></select
    ></label
  >
  <p>
    {t('Device:')} <code>{pending?.dev || device || t('not selected')}</code> · {t(
      groupNames[group],
    )}
  </p>
  {#if group === 'fan_and_lighting'}
    <label
      >{t('CPU temperature sensor for reset')}<select
        aria-label={t('CPU temperature sensor for reset')}
        bind:value={resetSensor}
        disabled={blocked}
      >
        <option value="">{t('Select a CPU temperature sensor')}</option>
        {#if resetSensor && !temperatureSensors.some((s) => s.id === resetSensor)}<option
            value={resetSensor}>{resetSensor} {t('· unavailable')}</option
          >{/if}
        {#each temperatureSensors as sensor (sensor.id)}<option value={sensor.id}
            >{sensor.label} · {sensor.id}</option
          >{/each}
      </select></label
    >
    <p>
      {t(
        'Full reset restores the standard 8 fan and 3 pump modes, their names and Fan Standard / Pump Quiet RPM selection, Rainbow, 360 mm/24 LED topology, and eight named fan/pump slots. Screen, files and the global thermal policy are preserved. An unavailable sensor uses 100% fallback.',
      )}
    </p>
    <button disabled={blocked || !resetSensor} onclick={() => start('reset')}
      >{t('Prepare full cooling and lighting reset…')}</button
    >
  {/if}
  <ExperimentalNotice
    feature="windows-import"
    message="Windows import supports a limited set of formats and is not verified for every L-Connect 2.1.28 export. Use at your own risk."
  />
  <div class="buttons">
    {#if group === 'fan_and_lighting'}<button disabled={blocked} onclick={() => start('windows')}
        >{t('Choose Windows ZIP…')}</button
      >{:else}<button disabled={blocked} onclick={() => start('windows_screen')}
        >{t('Choose Windows Screen ZIP…')}</button
      >{/if}<button disabled={blocked} onclick={() => start('export')}>{t('Export group…')}</button
    ><button disabled={blocked} onclick={() => start('import')}>{t('Choose group archive…')}</button
    >
  </div>
  <p>
    {t(
      'Export does not replace existing files. Selecting and inspecting an archive does not apply settings.',
    )}
  </p>
  {#if job}
    <p role="status">
      {job.mode === 'reset' && job.phase === 'prepared'
        ? '' + t('Reset prepared; waiting for confirmation') + ''
        : t(groupPhases[job.phase])}{job.file_name ? ` · ${job.file_name}` : ''}
    </p>
    {#if active}<progress
        aria-label={job.mode === 'reset' ? t('Reset operation') : t('Group transfer')}
        max={job.total || 1}
        value={job.total ? job.transferred : undefined}
      ></progress>{#if !['reset', 'windows'].includes(job.mode)}<p>
          {job.transferred.toLocaleString(presentation.language)} / {job.total
            ? job.total.toLocaleString(presentation.language)
            : '…'}
          {t('bytes')}
        </p>{/if}{/if}
    {#if job.mode === 'windows' && job.windows_source}
      <section class="windows-bindings" aria-label={t('Windows FanAndLighting bindings')}>
        <h3>{t('Source controller and sensors')}</h3>
        <p>
          {t(
            'Settings are applied to the device shown above. Choose a controller record from the archive and a separate sensor ID for each role. Unavailable IDs are retained without automatic substitution.',
          )}
        </p>
        <label
          >{t('Windows source controller')}<select
            aria-label={t('Windows source controller')}
            bind:value={windowsRecord}
            disabled={!windowsEditable}
            ><option value="">{t('Choose a controller record')}</option
            >{#each job.windows_source.controllers as source (source.record_index)}<option
                value={String(source.record_index)}
                >{t(
                  'Record {0} · {1}',
                  source.record_index,
                  source.profile_sha256.slice(0, 12),
                )}</option
              >{/each}</select
          ></label
        >
        <div class="windows-sources">
          {#each windowsRoles as role (role)}
            <div>
              <label
                >{windowsRoleLabel(role)} ({windowsUnitLabel(role)})<input
                  list={`windows-${job.id}-${role}`}
                  bind:value={windowsIds[role]}
                  aria-invalid={windowsBindingInvalid(role, windowsIds, sensors)}
                  aria-describedby={windowsBindingInvalid(role, windowsIds, sensors) ||
                  windowsSourceUnverified(role, windowsIds, sensors)
                    ? `windows-${job.id}-${role}-hint`
                    : undefined}
                  disabled={!windowsEditable}
                  maxlength="1024"
                  spellcheck="false"
                  placeholder={t('Choose or enter a sensor ID')}
                /></label
              >
              <datalist id={`windows-${job.id}-${role}`}
                >{#each sensors.filter((s) => s.unit === windowsUnit(role)) as sensor (sensor.id)}<option
                    value={sensor.id}>{sensor.label}</option
                  >{/each}</datalist
              >
              {#if windowsBindingInvalid(role, windowsIds, sensors)}<p
                  id={`windows-${job.id}-${role}-hint`}
                  role="alert"
                >
                  {t('The ID is duplicated, has incompatible units or uses an invalid format.')}
                </p>{:else if windowsSourceUnverified(role, windowsIds, sensors)}<p
                  id={`windows-${job.id}-${role}-hint`}
                  class="source-note"
                >
                  {t('Source unavailable or unverified. The ID is retained without substitution.')}
                </p>{/if}
            </div>
          {/each}
        </div>
        <p>
          {t(
            'Use six distinct IDs with the indicated units. Without temperature readings, cooling falls back to 100%; sensor-driven lighting returns to the effect’s manual settings.',
          )}
        </p>
        <button disabled={!windowsEditable || !windowsDraft} onclick={prepareWindows}
          >{t('Review Windows settings')}</button
        >
        {#if job.phase === 'prepared' && !windowsCurrent}<p role="status">
            {t('Bindings changed. Prepare again before applying.')}
          </p>{/if}
      </section>
    {/if}
    {#if job.mode === 'windows_screen' && job.windows_source}
      {#key job.id}<WindowsScreenBindings
          {job}
          {sensors}
          {fonts}
          editable={applicable}
          oninspect={(record) => prepareScreen({ record, inspect_only: true })}
          onprepare={prepareScreen}
        />{/key}
    {/if}
    {#if job.summary}
      <div aria-label={t('Group content')}>
        <p>{t('Slots:')} {job.summary.slots.length}</p>
        {#each job.summary.slots as slot (`${slot.group}:${slot.slot}`)}<p>
            {slot.group} · {slot.slot + 1}: {slot.name}
          </p>{/each}
        {#if job.summary.theme_name}<p>{t('Theme:')} {job.summary.theme_name}</p>{/if}
        {#if job.summary.media_key}<p>{t('Source: video')}</p>{/if}
        {#if job.summary.lcd}<p>
            LCD: {job.summary.lcd.brightness_percent}% · {job.summary.lcd.rotation_degrees}° · {job
              .summary.lcd.mode}
          </p>{/if}
        {#if job.summary.fan_manual_percent !== null}<p>
            {t('Fans:')}
            {job.summary.fan_manual_percent}%
          </p>{/if}
        {#if job.summary.pump_manual}<p>
            {t('Pump:')}
            {job.summary.pump_manual.percent}% · {job.summary.pump_manual.source}
          </p>{/if}
        {#if job.summary.cooling_modes}
          <details class="bank-summary">
            <summary
              >{t(
                'Stored modes: fans {0}, pump {1}',
                job.summary.cooling_modes.fan.length,
                job.summary.cooling_modes.pump.length,
              )}</summary
            >
            {#each job.summary.cooling_modes.fan as mode (mode.id)}<p>
                {t('Fans')}: {coolingModeLabel(mode)} ({mode.id}) · {policySummary(
                  mode.policy,
                  presentation.language,
                )}
              </p>{/each}
            {#each job.summary.cooling_modes.pump as mode (mode.id)}<p>
                {t('Pump')}: {coolingModeLabel(mode)} ({mode.id}) · {policySummary(
                  mode.policy,
                  presentation.language,
                )}
              </p>{/each}
          </details>
        {/if}
        {#if job.summary.cooling?.fan_mode || job.summary.cooling?.pump_mode}<p>
            {t(
              'Selected mode IDs: fans {0}, pump {1}',
              job.summary.cooling?.fan_mode ?? '—',
              job.summary.cooling?.pump_mode ?? '—',
            )}
          </p>{/if}
        {#if job.summary.cooling}<p>
            {t('Fan policy:')}
            {policySummary(job.summary.cooling.fan, presentation.language)}
          </p>
          <p>
            {t('Pump policy:')}
            {policySummary(job.summary.cooling.pump, presentation.language)}
          </p>{/if}
        {#if job.summary.dynamic_rgb}<p>
            {t('Dynamic lighting:')}
            {job.summary.dynamic_rgb.sensor_id} · {job.summary.dynamic_rgb.low_value}–{job.summary
              .dynamic_rgb.high_value}
            {job.summary.dynamic_rgb.unit}
          </p>{/if}
        {#if job.summary.fan_led_mode}<p>
            {t('Fan lighting:')}
            {job.summary.fan_led_mode}
          </p>{/if}
        {#if job.summary.pump_led_mode}<p>
            {t('Pump lighting:')}
            {job.summary.pump_led_mode}
          </p>{/if}
      </div>
    {/if}
    {#if job.windows_lighting}<WindowsLightingReview review={job.windows_lighting} />{/if}
    {#if job.mode === 'windows' && job.phase === 'prepared'}<p>
        {t(
          'Import replaces cooling and lighting modes. Saved Linux profiles, display settings, files and thermal alerts are preserved. Application starts after confirmation.',
        )}
      </p>
      <button disabled={!applicable || !windowsCurrent} onclick={() => act('finish')}
        >{t('Import and apply')}</button
      >{/if}
    {#if job.mode === 'windows_screen' && job.phase === 'prepared'}<WindowsScreenPreview
        target={job}
        {geometry}
        enabled={applicable}
        onbusy={(value) => (screenPreviewBusy = value)}
      />{/if}
    {#if job.mode === 'windows_screen' && job.phase === 'prepared'}<p>
        {t(
          'Import replaces the current LCD scene, editor, brightness, rotation and galleries. Saved Linux profiles and cooling settings are preserved. Physical output is verified separately.',
        )}
      </p>
      <button disabled={!applicable || screenPreviewBusy} onclick={() => act('finish')}
        >{t('Import screen and apply')}</button
      >{/if}
    {#if job.phase === 'prepared' && !['windows', 'windows_screen'].includes(job.mode)}<p>
        {job.mode === 'reset' ? '' + t('Reset') + '' : '' + t('Restore') + ''}
        {t(
          'will replace the saved settings and slots of the entire group and immediately apply them to the specified device. The other group and open editor drafts are preserved.',
        )}
      </p>
      <button disabled={!applicable} onclick={() => act('finish')}
        >{job.mode === 'reset'
          ? '' + t('Confirm full reset and apply') + ''
          : '' + t('Restore and apply group') + ''}</button
      >{/if}
    {#if job.mode === 'reset' && job.phase === 'uncertain' && job.snapshot && job.sensor_id}<p>
        {t(
          'The reset response is unconfirmed. Retry uses the same ID, sensor and original settings revision; a new reset is never started automatically.',
        )}
      </p>
      <button disabled={!applicable} onclick={() => act('finish')}
        >{t('Retry reset with the same ID')}</button
      >{/if}
    {#if job.mode === 'windows' && job.phase === 'uncertain' && job.snapshot && job.windows_options}<p
      >
        {t(
          'The import reply is unconfirmed. Retry uses the prepared settings and the same operation ID; form edits are not included.',
        )}
      </p>
      <button disabled={!applicable} onclick={() => act('finish')}
        >{t('Retry import with the same ID')}</button
      >{/if}
    {#if job.phase === 'applying'}<p>
        {t(
          'Applying has already started. Wait for the result; if disconnected, read it back using the operation ID.',
        )}
      </p>{/if}
    {#if job.phase === 'complete' && job.mode === 'export'}<p role="status">
        {t('Group archive saved to a file.')}
      </p>{/if}
    {#if job.readback}
      <p role="status">
        {t('Group save confirmed.')}
        {job.readback.selected_content_current
          ? '' + t('The saved group content matches this operation.') + ''
          : '' + t('Settings have changed since this operation.') + ''}
      </p>
      {#if job.readback.runtime.recovery_required}<p role="alert">
          {t('Runtime recovery is required. Restart the daemon and read the result again.')}
        </p>{/if}
      {#if job.readback.runtime.last_error}<p role="alert">
          {t('Settings were saved, but applying failed:')}
          {job.readback.runtime.last_error}
        </p>{/if}
      <button
        disabled={!applicable ||
          screenPreviewBusy ||
          !job.readback.selected_content_current ||
          !job.readback.runtime.revision ||
          !!job.readback.runtime.recovery_required}
        onclick={() => act('reapply')}>{t('Reapply saved group')}</button
      >
    {/if}
    {#if job.message || job.message_code}<p
        role={['failed', 'uncertain'].includes(job.phase) ? 'alert' : 'status'}
      >
        {archiveMessage(job, presentation.language)}
      </p>
      {#if archiveDiagnostic(job, presentation.language)}<details>
          <summary>{t('Technical diagnostics')}</summary>
          <pre>{archiveDiagnostic(job, presentation.language)}</pre>
        </details>{/if}{/if}
    {#if cancellable}<button onclick={() => act('cancel')}
        >{job.mode === 'windows'
          ? t('Cancel Windows preparation')
          : job.mode === 'reset'
            ? '' + t('Cancel reset preparation') + ''
            : '' + t('Cancel group transfer') + ''}</button
      >{/if}
  {/if}
  <label
    >{t('Group operation ID')}<input
      bind:value={recoveryId}
      maxlength="32"
      spellcheck="false"
      disabled={active || !!pending}
    /></label
  >
  <label
    >{t('Device for result readback')}<input
      bind:value={recoveryDev}
      placeholder={device}
      disabled={active || !!pending}
    /></label
  >
  <label
    >{t('Operation type')}<select
      aria-label={t('Operation type')}
      bind:value={recoveryMode}
      disabled={active || !!pending}
      ><option value="import">{t('Import')}</option><option value="export">{t('Export')}</option
      ><option value="reset">{t('Full Fan + Lighting reset')}</option><option value="windows"
        >{t('Windows FanAndLighting import')}</option
      ><option value="windows_screen">{t('Import Windows Screen')}</option></select
    ></label
  >
  <div class="buttons">
    <button disabled={active || !connected || !recoveryValid} onclick={() => act('recover')}
      >{t('Read group result')}</button
    >
    {#if pending || job}<button
        disabled={active || job?.phase === 'prepared'}
        onclick={() => act('dismiss')}>{t('Close group details')}</button
      >{/if}
    {#if pending && !job && !['reset', 'windows'].includes(pending.mode)}<button
        disabled={active || !connected}
        onclick={() => act('cancel')}>{t('Cancel saved transfer')}</button
      >{/if}
  </div>
  {#if pending}<p>
      {t('ID saved for recovery:')} <code>{pending.id}</code>{t(
        '. Closing these details does not cancel an accepted operation.',
      )}
    </p>{/if}
  {#if error}<p role="alert">{archiveMessage(error, presentation.language)}</p>
    {#if archiveDiagnostic(error, presentation.language)}<details>
        <summary>{t('Technical diagnostics')}</summary>
        <pre>{archiveDiagnostic(error, presentation.language)}</pre>
      </details>{/if}{/if}
</details>

<style>
  .windows-sources {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0 18px;
  }
  .source-note {
    color: var(--text-muted);
    font-size: 12px;
  }
  .windows-bindings {
    border-top: 1px solid var(--border);
    padding-top: 20px;
    margin-top: 20px;
  }
  .windows-sources input {
    min-width: 0;
    width: 100%;
  }
  @media (max-width: 800px) {
    .windows-sources {
      grid-template-columns: 1fr;
    }
  }
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
  details {
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    margin-block: 16px;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  p {
    line-height: 1.5;
  }
  label {
    display: grid;
    gap: 6px;
    margin: 12px 0;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  progress {
    width: 100%;
  }
  [role='alert'] {
    color: var(--danger);
  }
  code {
    user-select: all;
    overflow-wrap: anywhere;
  }
</style>
