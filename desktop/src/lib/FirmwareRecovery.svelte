<script lang="ts">
  import { onDestroy } from 'svelte';
  import { t } from './i18n.svelte';
  import { rpc, message } from './api';
  import {
    firmwareRecovery,
    type FirmwareRecovery,
    type RecoveryState,
    type SavedStage,
  } from './firmware-recovery';
  let snapshot = $state<FirmwareRecovery | null>(null),
    busy = $state(false),
    failure = $state<'read' | 'invalid' | null>(null),
    diagnostic = $state('');
  let sequence = 0,
    destroyed = false;
  onDestroy(() => {
    destroyed = true;
    sequence++;
  });
  async function read() {
    if (busy || destroyed) return;
    const serial = ++sequence;
    const current = () => !destroyed && serial === sequence;
    busy = true;
    snapshot = null;
    failure = null;
    diagnostic = '';
    try {
      const response = await rpc<unknown>('get_firmware_recovery');
      if (!current()) return;
      const parsed = firmwareRecovery(response);
      if (!parsed) {
        failure = 'invalid';
        diagnostic = JSON.stringify(response)?.slice(0, 16384) ?? '';
        return;
      }
      snapshot = parsed;
    } catch (error) {
      if (current()) {
        failure = 'read';
        diagnostic = message(error);
      }
    } finally {
      if (current()) busy = false;
    }
  }
  function stateLabel(state: RecoveryState) {
    switch (state) {
      case 'no_saved_operation':
        return t('No saved firmware journal or guards were found.');
      case 'journal_only':
        return t(
          'An unresolved firmware journal exists. Missing markers do not prove that no device writes occurred.',
        );
      case 'guarded':
        return t(
          'A saved firmware operation has quarantined settings replay. Target verification and operation resolution are required.',
        );
      case 'guard_only':
        return t(
          'A settings guard exists without a firmware journal. The operation outcome is unknown.',
        );
      case 'inconsistent':
        return t(
          'The firmware journal and settings guards disagree. The operation outcome is unknown.',
        );
      case 'unreadable':
        return t('Saved firmware state could not be read. The operation outcome is unknown.');
    }
  }
  function stageLabel(stage: SavedStage) {
    switch (stage) {
      case 'armed':
        return t('Saved: operation armed');
      case 'effects_possible':
        return t('Saved: commands may have changed the device');
      case 'not_started':
        return t('Saved: transfer not started');
      case 'unknown':
        return t('Saved: outcome unknown');
      case 'awaiting_reconnect':
        return t('Saved: awaiting reconnect');
    }
  }
  const target = (value: number) => value.toString(16).padStart(4, '0');
</script>

<section aria-label={t('Saved firmware state')} aria-busy={busy}>
  <h2>{t('Saved firmware state')}</h2>
  <p>
    {t(
      'This is a snapshot of saved files, not live status or confirmation of a successful update. Read again for a new snapshot.',
    )}
  </p>
  <p>
    {t(
      'Reading is available without a connected device and while ordinary daemon commands are blocked.',
    )}
  </p>
  <button aria-disabled={busy} onclick={read}>{t('Read saved firmware state')}</button>
  {#if busy}<p role="status">{t('Reading saved firmware state…')}</p>
  {:else if failure}<p role="alert">
      {failure === 'invalid'
        ? t('The daemon returned unsupported saved firmware state.')
        : t('Could not read saved firmware state. Read again explicitly.')}
    </p>
  {:else if snapshot}<p role={snapshot.state === 'no_saved_operation' ? 'status' : 'alert'}>
      {stateLabel(snapshot.state)}
    </p>
  {:else}<p role="status">{t('Saved firmware state has not been read yet.')}</p>{/if}
  {#if snapshot}
    {#if snapshot.state !== 'no_saved_operation'}<p>
        {t(
          'Preserve the journal and guards for diagnosis. This read does not clear quarantine, retry the transfer or replay device settings.',
        )}
      </p>{/if}
    {#if snapshot.operation}
      {@const operation = snapshot.operation}
      <dl>
        <dt>{t('Saved firmware operation ID')}</dt>
        <dd><code>{operation.operation_id}</code></dd>
        <dt>{t('Saved stage')}</dt>
        <dd>{stageLabel(operation.stage)}</dd>
        <dt>{t('Saved target VID:PID')}</dt>
        <dd><code>{target(operation.vid)}:{target(operation.pid)}</code></dd>
        <dt>{t('Saved serial number')}</dt>
        <dd>{operation.serial ?? t('Serial number was not saved')}</dd>
        <dt>{t('Previous HID path')}</dt>
        <dd><code>{operation.previous_hid_path}</code></dd>
        <dt>{t('Saved settings key')}</dt>
        <dd><code>{operation.settings_key}</code></dd>
        <dt>{t('Saved image SHA-256')}</dt>
        <dd><code>{operation.image_sha256}</code></dd>
      </dl>
      {#if operation.progress}
        {@const progress = operation.progress}
        <h3>{t('Saved transfer markers')}</h3>
        <p>
          {t(
            'After a crash, saved markers may lag actual commands. They do not prove completion or the absence of other writes.',
          )}
        </p>
        <dl>
          <dt>{t('Chunks in the saved plan')}</dt>
          <dd>{progress.total_chunks}</dd>
          <dt>{t('Saved chunk-write markers')}</dt>
          <dd>{progress.chunk_writes}</dd>
          <dt>{t('Saved chunk-echo markers')}</dt>
          <dd>{progress.chunk_echoes}</dd>
          <dt>{t('Pump command marker')}</dt>
          <dd>{progress.pump_hold_written ? t('Marker saved') : t('Marker not saved')}</dd>
          <dt>{t('Fan command marker')}</dt>
          <dd>{progress.fan_hold_written ? t('Marker saved') : t('Marker not saved')}</dd>
          <dt>{t('Activation command marker')}</dt>
          <dd>
            {progress.activation_written ? t('Marker saved') : t('Marker not saved')}
          </dd>
        </dl>
      {/if}
    {/if}
    {#if snapshot.guards.length}
      <h3>{t('Saved settings guards')}</h3>
      <ul>
        {#each snapshot.guards as guard (guard.settings_key)}<li>
            <dl>
              <dt>{t('Saved settings key')}</dt>
              <dd><code>{guard.settings_key}</code></dd>
              <dt>{t('Saved firmware operation ID')}</dt>
              <dd><code>{guard.operation_id}</code></dd>
              <dt>{t('Saved image SHA-256')}</dt>
              <dd><code>{guard.image_sha256}</code></dd>
            </dl>
          </li>{/each}
      </ul>
    {/if}
    <details>
      <summary>{t('Saved firmware diagnostics')}</summary><textarea
        readonly
        rows="12"
        aria-label={t('Saved firmware diagnostics')}
        value={JSON.stringify(snapshot, null, 2)}></textarea>
    </details>
  {/if}
  {#if diagnostic}<details>
      <summary>{t('Technical diagnostics')}</summary>
      <pre>{diagnostic}</pre>
    </details>{/if}
</section>

<style>
  section {
    max-width: 760px;
    display: grid;
    gap: 14px;
    margin-block: 20px;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 12px;
  }
  h2 {
    font-size: 19px;
  }
  h3 {
    font-size: 16px;
  }
  p {
    line-height: 1.6;
    color: var(--text-muted);
  }
  button {
    justify-self: start;
  }
  button[aria-disabled='true'] {
    opacity: 0.6;
  }
  dl {
    display: grid;
    grid-template-columns: minmax(100px, 1fr) minmax(0, 2fr);
    gap: 10px;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  ul {
    padding-left: 20px;
  }
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  textarea {
    box-sizing: border-box;
    width: 100%;
    font-family: monospace;
    background: var(--canvas);
    color: inherit;
    resize: vertical;
  }
  button:focus-visible,
  textarea:focus-visible,
  summary:focus-visible {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }
  @media (max-width: 540px) {
    dl {
      grid-template-columns: 1fr;
    }
  }
</style>
