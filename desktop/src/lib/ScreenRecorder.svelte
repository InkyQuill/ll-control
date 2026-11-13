<script lang="ts">
  import { t } from './i18n.svelte';
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { message } from './api';
  import {
    RecordingSession,
    RecordingReview,
    type RecordingStatus,
    type RecordingReviewStatus,
    type RecordingAction,
  } from './screen-record';
  import RecordingRegionPreview from './RecordingRegionPreview.svelte';
  import PreparedVideoPreview from './PreparedVideoPreview.svelte';
  import VideoSourcePreview from './VideoSourcePreview.svelte';
  import { normalizedCrop, type CropFields } from './video-crop';
  let {
    device,
    onclose,
    onsave,
  }: {
    device: string;
    onclose: () => void;
    onsave: (key: string, target: string) => Promise<void>;
  } = $props();
  let capture = $state<RecordingSession | null>(null),
    review = $state<RecordingReview | null>(null);
  let status = $state<RecordingStatus | null>(null),
    result = $state<RecordingReviewStatus | null>(null);
  type RecordingFailure = { detail: string; context: 'direct' | 'return' | 'close' };
  let cleanupRequested = $state(false);
  let busy = $state(false),
    closing = $state(false),
    returning = $state(false),
    error = $state<RecordingFailure | null>(null),
    sourceUrl = $state('');
  const blocked = $derived(busy || closing || cleanupRequested);
  function setError(cause: unknown, context: RecordingFailure['context'] = 'direct') {
    error = { detail: message(cause), context };
  }
  let name = $state(t('Screen recording')),
    fit = $state<'contain' | 'cover' | 'stretch'>('contain'),
    clearAsked = $state(false);
  let heading = $state<HTMLHeadingElement>();
  const fullRegion = () => ({ x: 0, y: 0, width: 100, height: 100 });
  let captureRegion = $state<CropFields>(fullRegion());
  const requestedRegion = $derived(normalizedCrop(captureRegion));
  const appliedRegion = $derived(status?.region ?? { x: 0, y: 0, width: 1, height: 1 });
  const regionDirty = $derived.by(() => {
    const requested = requestedRegion;
    // Percentage round trips can differ by a few ulps, far below a source pixel.
    return (
      !requested ||
      (['x', 'y', 'width', 'height'] as const).some(
        (key) => Math.abs(requested[key] - appliedRegion[key]) > 1e-9,
      )
    );
  });
  function useAppliedRegion() {
    captureRegion = {
      x: appliedRegion.x * 100,
      y: appliedRegion.y * 100,
      width: appliedRegion.width * 100,
      height: appliedRegion.height * 100,
    };
  }
  async function applyRegion() {
    if (
      !status ||
      !['setup', 'paused'].includes(status.phase) ||
      !requestedRegion ||
      !regionDirty ||
      blocked
    )
      return;
    await control({ set_region: requestedRegion });
  }

  type Fragment = {
    id: number;
    start: number | undefined;
    end: number | undefined;
    region: CropFields;
  };
  let nextFragment = 1;
  const freshFragment = (end: number | undefined): Fragment => ({
    id: nextFragment++,
    start: 0,
    end,
    region: { x: 0, y: 0, width: 100, height: 100 },
  });
  let editEnabled = $state(false),
    fragments = $state<Fragment[]>([freshFragment(undefined)]),
    selectedId = $state(1);
  const selected = $derived(fragments.find((fragment) => fragment.id === selectedId)!);
  let position = $state<number | null>(null);
  const crop = $derived(normalizedCrop(selected.region));
  const sourceDuration = $derived(status?.metadata?.durationSeconds ?? null);
  const cut = $derived({
    segments: fragments.map((fragment) => ({
      start_ms: Math.round(fragment.start! * 1000),
      end_ms: Math.round(fragment.end! * 1000),
      crop: normalizedCrop(fragment.region)!,
    })),
  });
  const cutDuration = $derived(
    cut.segments.reduce((sum, fragment) => sum + fragment.end_ms - fragment.start_ms, 0),
  );
  const validCut = $derived(
    !editEnabled ||
      (sourceDuration !== null &&
        fragments.length > 0 &&
        fragments.length <= 64 &&
        cutDuration <= 180000 &&
        fragments.every(
          (fragment) =>
            normalizedCrop(fragment.region) !== null &&
            fragment.start !== undefined &&
            fragment.end !== undefined &&
            Number.isFinite(fragment.start) &&
            Number.isFinite(fragment.end) &&
            fragment.start >= 0 &&
            fragment.end <= sourceDuration! &&
            Math.round(fragment.end * 1000) > Math.round(fragment.start * 1000),
        )),
  );
  function toggleEdit() {
    if (editEnabled && fragments.length === 1 && selected.end === undefined)
      selected.end = sourceDuration ?? undefined;
  }
  function mark(edge: 'start' | 'end') {
    if (position !== null) selected[edge] = position;
  }
  function addFragment() {
    if (fragments.length >= 64) return;
    const fragment = freshFragment(sourceDuration ?? undefined);
    fragments.push(fragment);
    selectedId = fragment.id;
  }
  function removeFragment() {
    if (fragments.length <= 1) return;
    const index = fragments.findIndex((fragment) => fragment.id === selectedId);
    const keep = fragments.filter((fragment) => fragment.id !== selectedId);
    selectedId = keep[Math.min(index, keep.length - 1)].id;
    fragments = keep;
  }
  function moveFragment(direction: number) {
    const index = fragments.findIndex((fragment) => fragment.id === selectedId),
      target = index + direction;
    if (target < 0 || target >= fragments.length) return;
    const copy = [...fragments];
    [copy[index], copy[target]] = [copy[target], copy[index]];
    fragments = copy;
  }
  let limitDialog = $state<HTMLDialogElement>(),
    finishButton = $state<HTMLButtonElement>();
  let offeredLimit: string | null = null;
  let operationReason = $state<'manual' | 'limit'>('manual');
  let pendingOperation: 'edit' | 'import' | null = null;
  const limitTitle = $props.id();
  async function offerLimit(id: string, epoch: number) {
    offeredLimit = id;
    operationReason = 'limit';
    await tick();
    if (
      mounted &&
      epoch === generation &&
      !closing &&
      status?.id === id &&
      status.phase === 'limit_reached' &&
      !limitDialog?.open
    )
      limitDialog?.showModal();
  }
  function dismissLimit() {
    limitDialog?.close();
    finishButton?.focus();
  }
  async function offerOperations() {
    if (blocked || !capture) return;
    const epoch = generation;
    if (status?.phase === 'recording') await control('pause');
    if (
      !mounted ||
      epoch !== generation ||
      closing ||
      !status ||
      !['paused', 'limit_reached'].includes(status.phase)
    )
      return;
    operationReason = status.phase === 'limit_reached' ? 'limit' : 'manual';
    await tick();
    if (mounted && epoch === generation && !closing) limitDialog?.showModal();
  }
  async function acceptOperation(operation: 'edit' | 'import') {
    if (blocked || !status || !['paused', 'limit_reached'].includes(status.phase)) return;
    pendingOperation = operation;
    dismissLimit();
    await control('finish');
  }
  let mounted = false,
    generation = 0,
    refreshRevision = 0,
    announced: string | null = null;
  const labels: Record<RecordingStatus['phase'], string> = {
    reserved: 'Preparing recording',
    selecting: 'Select a screen or window',
    setup: 'Select a region before recording',
    recording: 'Recording',
    paused: 'Recording paused',
    limit_reached: '180-second limit reached',
    finalizing: 'Finishing recording',
    ready: 'Recording ready',
    cancelling: 'Cancelling recording',
    cancelled: 'Recording cancelled',
    failed: 'Recording incomplete',
  };
  function recordingError(value: string | null | undefined): string {
    const text = value ?? '';
    if (
      text.startsWith('Recording worker response timed out') ||
      text.startsWith('Recording worker exit timed out')
    )
      return t(
        'The recording process is not responding. Recording stopped. Select the source again.',
      );
    if (
      text.startsWith('Recording worker connection closed') ||
      text.startsWith('Connection reset by peer')
    )
      return t('The recording process exited. Select the source again.');
    if (
      text.startsWith('Invalid recording worker message') ||
      text.startsWith('Invalid recording startup acknowledgement') ||
      text.startsWith('Invalid recording status reply') ||
      text.startsWith('Invalid recording command acknowledgement') ||
      text.startsWith('Invalid recording snapshot reply') ||
      text.startsWith('Invalid recording finish reply')
    )
      return t('The recording process returned an invalid response. Select the source again.');
    if (
      text.startsWith('ScreenCast session closed while recording') ||
      text.startsWith('PipeWire connection closed while recording')
    )
      return t('The screen capture session closed. Select the source again.');
    if (text === 'Recording cancelled') return t('Recording cancelled');
    if (text.startsWith('No source frame yet'))
      return t('The source has not supplied a frame yet. Refresh the frame and try again.');
    if (text.startsWith('Capture source ended unexpectedly'))
      return t('The capture source stopped sending frames. Select the source again.');
    if (text.startsWith('Recording time limit reached')) return t('180-second limit reached');
    if (text.startsWith('Recording contains no frames'))
      return t('The recording contains no frames. Select the source again.');
    if (
      text.startsWith('Recording exceeds output size limit') ||
      text.startsWith('Recording exceeds file size limit')
    )
      return t(
        'The recording exceeds the size limit. Select a smaller region or make a shorter recording.',
      );
    if (text.startsWith('Pause recording before changing capture region'))
      return t('Pause recording before changing the capture region.');
    if (text.startsWith('ScreenCast selection timed out'))
      return t('Source selection timed out. Select the screen or window again.');
    if (text.startsWith('ScreenCast portal rejected request (1)'))
      return t('Source selection was cancelled.');
    if (text.startsWith('PipeWire plugin lacks'))
      return t(
        'The installed PipeWire plugin does not support screen recording. Update the GStreamer plugin for PipeWire.',
      );
    if (text.startsWith('Start isolated recording worker'))
      return t('Could not start the recording process. Select the source again.');
    // Unclassified diagnostics are data, even if they equal a resource key.
    return text;
  }
  function failureText(): string {
    if (error?.context === 'return')
      return t(
        'Could not discard the prepared version: {0}. Try returning to editing again.',
        recordingError(error.detail),
      );
    if (error?.context === 'close')
      return t('Could not confirm cleanup: {0}. Try closing again.', recordingError(error.detail));
    return recordingError(error?.detail ?? status?.message);
  }
  async function refresh() {
    if (cleanupRequested) return;
    const epoch = generation,
      revision = ++refreshRevision,
      owner = capture,
      output = review;
    const current = () => mounted && epoch === generation && revision === refreshRevision;
    try {
      const next = await owner?.refresh();
      if (!current()) return;
      status = next ?? null;
      if (next?.phase === 'limit_reached' && offeredLimit !== next.id)
        void offerLimit(next.id, epoch);
      if (!next || !['paused', 'limit_reached'].includes(next.phase)) limitDialog?.close();
      if (next?.phase === 'ready' && !sourceUrl && owner) {
        const url = await owner.preview();
        if (!current()) return;
        sourceUrl = url;
      }
      if (next?.phase === 'ready' && pendingOperation && !busy && !closing) {
        const operation = pendingOperation;
        pendingOperation = null;
        if (operation === 'import') {
          fit = 'stretch';
          await prepare();
          return;
        }
        await tick();
        if (current()) heading?.focus();
      }
      if (returning) return;
      const nextResult = await output?.refresh();
      if (!current() || output !== review) return;
      result = nextResult ?? null;
      if (nextResult?.phase === 'complete' && nextResult.key && announced !== nextResult.id) {
        announced = nextResult.id;
        await onsave(nextResult.key, device);
      }
    } catch (e) {
      if (current()) setError(e);
    }
  }
  async function start() {
    if (blocked || capture) return;
    const epoch = generation;
    busy = true;
    error = null;
    const owner = new RecordingSession(crypto.randomUUID().replaceAll('-', ''), invoke);
    capture = owner;
    try {
      const next = await owner.begin();
      if (mounted && epoch === generation) status = next;
    } catch (e) {
      if (mounted && epoch === generation) setError(e);
    } finally {
      if (mounted && epoch === generation) {
        busy = false;
        await refresh();
      }
    }
  }
  async function control(action: RecordingAction) {
    if (!capture || blocked) return;
    refreshRevision++;
    const epoch = generation;
    busy = true;
    error = null;
    try {
      await capture.control(action);
    } catch (e) {
      if (mounted && epoch === generation) setError(e);
    } finally {
      if (mounted && epoch === generation) {
        busy = false;
        await refresh();
      }
    }
  }
  async function prepare() {
    if (blocked || review || status?.phase !== 'ready' || !capture || !name.trim() || !validCut)
      return;
    const epoch = generation;
    busy = true;
    error = null;
    const output = new RecordingReview(crypto.randomUUID().replaceAll('-', ''), capture.id, invoke);
    review = output;
    try {
      await output.prepare({ name: name.trim() + '.mp4', fit, cut: editEnabled ? cut : null });
    } catch (e) {
      if (mounted && epoch === generation) setError(e);
    } finally {
      if (mounted && epoch === generation) {
        busy = false;
        await refresh();
      }
    }
  }
  const canReturnToEdit = $derived(
    !!review &&
      (['review_ready', 'failed', 'cancelled'].includes(result?.phase ?? '') ||
        (!!error && !['publishing', 'complete'].includes(result?.phase ?? ''))),
  );
  async function returnToEdit() {
    if (!review || blocked || !canReturnToEdit) return;
    const epoch = ++generation,
      output = review;
    returning = true;
    busy = true;
    error = null;
    try {
      await output.dispose();
      if (!mounted || epoch !== generation) return;
      review = null;
      result = null;
      returning = false;
      await tick();
      heading?.focus();
    } catch (e) {
      if (mounted && epoch === generation) setError(e, 'return');
    } finally {
      if (mounted && epoch === generation) busy = false;
    }
  }
  async function save() {
    if (!review || blocked || returning || result?.phase !== 'review_ready') return;
    const epoch = generation;
    busy = true;
    error = null;
    try {
      await review.confirm();
    } catch (e) {
      if (mounted && epoch === generation) setError(e);
    } finally {
      if (mounted && epoch === generation) {
        busy = false;
        await refresh();
      }
    }
  }
  async function release() {
    // Both leases must be attempted even if one reports an ambiguous cleanup.
    const outcomes = await Promise.allSettled([review?.dispose(), capture?.dispose()]);
    const failure = outcomes.find(
      (item): item is PromiseRejectedResult => item.status === 'rejected',
    );
    if (failure) throw failure.reason;
  }
  async function close(clear = false) {
    if (closing) return;
    cleanupRequested = true;
    closing = true;
    generation++;
    pendingOperation = null;
    error = null;
    limitDialog?.close();
    try {
      await release();
      if (!mounted) return;
      if (clear) {
        cleanupRequested = false;
        captureRegion = fullRegion();
        capture = null;
        review = null;
        status = null;
        result = null;
        sourceUrl = '';
        clearAsked = false;
        announced = null;
        offeredLimit = null;
        busy = false;
        returning = false;
        editEnabled = false;
        fragments = [freshFragment(undefined)];
        selectedId = fragments[0].id;
        position = null;
      } else onclose();
    } catch (e) {
      if (mounted) setError(e, 'close');
    } finally {
      if (mounted) closing = false;
    }
  }
  onMount(() => {
    mounted = true;
    heading?.focus();
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      if (!closing) await refresh();
      if (mounted) timer = setTimeout(poll, 400);
    }
    void poll();
    return () => {
      mounted = false;
      generation++;
      clearTimeout(timer);
      void release().catch((e) => console.error('Recording cleanup failed', e));
    };
  });
</script>

<dialog
  bind:this={limitDialog}
  aria-labelledby={limitTitle}
  oncancel={(event) => {
    event.preventDefault();
    dismissLimit();
  }}
>
  <h3 id={limitTitle}>
    {operationReason === 'limit'
      ? '' + t('Recording limit reached') + ''
      : '' + t('What would you like to do with the recording?') + ''}
  </h3>
  <p>
    {operationReason === 'limit' ? '' + t('180 seconds recorded.') + ' ' : ''}{t(
      'Recording is paused. You can edit it or prepare the entire recording for the LCD.',
    )}
  </p>
  <button disabled={blocked} onclick={() => acceptOperation('edit')}>{t('Edit recording')}</button>
  <button disabled={blocked} onclick={() => acceptOperation('import')}
    >{t('Prepare entire recording')}</button
  >
  <button disabled={closing} onclick={dismissLimit}>{t('Return to recording')}</button>
</dialog>
<section aria-label={t('Screen recording')}>
  <h3 bind:this={heading} tabindex="-1">{t('Screen recording')}</h3>
  <p>
    {t(
      'Select a screen or window in the system dialog. Audio is not recorded. Pauses do not count towards the 180-second limit.',
    )}
  </p>
  <p>
    {t(
      'Select a region before recording, or use the entire source. While paused, you can change the region for subsequent frames.',
    )}
  </p>
  {#if status}<p role="status">
      {t(labels[status.phase])} · {(status.recordedMillis / 1000).toFixed(1)}
      {t('s')}
    </p>{/if}
  {#if !capture}<button disabled={blocked} onclick={start}>{t('Select screen or window')}</button
    >{/if}
  {#if status?.phase === 'setup'}<button
      disabled={blocked || regionDirty}
      onclick={() => control('resume')}>{t('Start screen recording')}</button
    >{/if}
  {#if status?.phase === 'recording'}<button disabled={blocked} onclick={() => control('pause')}
      >{t('Pause the recording')}</button
    >{/if}
  {#if status?.phase === 'paused'}<button
      disabled={blocked || regionDirty}
      onclick={() => control('resume')}>{t('Resume recording')}</button
    >{/if}
  {#if status && ['setup', 'recording', 'paused', 'limit_reached'].includes(status.phase)}
    <p aria-label={t('Applied capture region')}>
      {t('Region: X')}
      {(appliedRegion.x * 100).toFixed(1)}%, Y {(appliedRegion.y * 100).toFixed(1)}{t('%, width')}
      {(appliedRegion.width * 100).toFixed(1)}{t('%, height')}
      {(appliedRegion.height * 100).toFixed(1)}%.
    </p>
  {/if}
  {#if status && ['setup', 'paused'].includes(status.phase) && capture}
    {#key capture.id}<RecordingRegionPreview
        session={capture}
        crop={requestedRegion}
        disabled={blocked}
        onchange={(region) =>
          (captureRegion = {
            x: region.x * 100,
            y: region.y * 100,
            width: Math.min(region.width * 100, 100 - region.x * 100),
            height: Math.min(region.height * 100, 100 - region.y * 100),
          })}
      />{/key}
    <fieldset disabled={blocked}>
      <legend
        >{status.phase === 'setup'
          ? '' + t('Recording region') + ''
          : '' + t('Region for the next interval') + ''}</legend
      >
      <p>
        {t(
          'Coordinates are percentages of the selected screen or window. The region is stretched to the fixed recording size.',
        )}
      </p>
      <label
        >{t('Capture X, %')}<input
          type="number"
          min="0"
          max="100"
          step="0.1"
          bind:value={captureRegion.x}
        /></label
      >
      <label
        >{t('Capture Y, %')}<input
          type="number"
          min="0"
          max="100"
          step="0.1"
          bind:value={captureRegion.y}
        /></label
      >
      <label
        >{t('Capture width, %')}<input
          type="number"
          min="0.1"
          max="100"
          step="0.1"
          bind:value={captureRegion.width}
        /></label
      >
      <label
        >{t('Capture height, %')}<input
          type="number"
          min="0.1"
          max="100"
          step="0.1"
          bind:value={captureRegion.height}
        /></label
      >
      {#if !requestedRegion}<p role="status">
          {t('The region must be nonempty and stay within the source.')}
        </p>
      {:else if regionDirty}<p role="status">
          {t('Apply the region or restore confirmed values before starting or resuming recording.')}
        </p>{/if}
      <button disabled={!requestedRegion || !regionDirty} onclick={applyRegion}
        >{t('Apply capture region')}</button
      >
      <button onclick={() => (captureRegion = fullRegion())}>{t('Entire source')}</button>
      <button disabled={!regionDirty} onclick={useAppliedRegion}
        >{t('Restore applied region')}</button
      >
    </fieldset>
  {/if}
  {#if status && ['recording', 'paused', 'limit_reached'].includes(status.phase)}<button
      bind:this={finishButton}
      disabled={blocked}
      onclick={offerOperations}>{t('Finish and review recording')}</button
    >{/if}
  {#if sourceUrl && status?.phase === 'ready' && !review}
    <VideoSourcePreview
      external={{ url: sourceUrl, name: t('Screen recording') }}
      disabled={blocked}
      crop={editEnabled ? crop : null}
      {fit}
      onposition={(value) => (position = value)}
      onmetadata={() => {}}
      oncrop={(value) =>
        (selected.region = {
          x: value.x * 100,
          y: value.y * 100,
          width: value.width * 100,
          height: value.height * 100,
        })}
    />
    <fieldset disabled={blocked}>
      <legend>{t('Recording edits')}</legend>
      <label
        ><input type="checkbox" bind:checked={editEnabled} onchange={toggleEdit} />{t(
          'Trim prepared recording',
        )}</label
      >
      {#if editEnabled}
        <label for={limitTitle + '-fragment'}>{t('Recording segment')}</label><select
          id={limitTitle + '-fragment'}
          bind:value={selectedId}
          >{#each fragments as fragment, index (fragment.id)}<option value={fragment.id}
              >{t('Segment')} {index + 1}</option
            >{/each}</select
        >
        <button disabled={fragments.length >= 64} onclick={addFragment}
          >{t('Add recording segment')}</button
        >
        <button disabled={fragments.length <= 1} onclick={removeFragment}
          >{t('Delete recording segment')}</button
        >
        <button disabled={selectedId === fragments[0].id} onclick={() => moveFragment(-1)}
          >{t('Move segment earlier')}</button
        >
        <button
          disabled={selectedId === fragments[fragments.length - 1].id}
          onclick={() => moveFragment(1)}>{t('Move segment later')}</button
        >
        <p>
          {t('Segments are joined in this order. Total length:')}
          {Number.isFinite(cutDuration) ? (cutDuration / 1000).toFixed(3) : '—'}
          {t('s / 180 s.')}
        </p>
        <label
          >{t('Recording start, s')}<input
            type="number"
            min="0"
            step="0.001"
            bind:value={selected.start}
          /></label
        >
        <button disabled={position === null} onclick={() => mark('start')}
          >{t('Set start to current frame')}</button
        >
        <label
          >{t('Recording end, s')}<input
            type="number"
            min="0"
            step="0.001"
            max={sourceDuration ?? undefined}
            bind:value={selected.end}
          /></label
        >
        <button disabled={position === null} onclick={() => mark('end')}
          >{t('Set end to current frame')}</button
        >
        <label
          >{t('Region X, %')}<input
            type="number"
            min="0"
            max="100"
            step="0.1"
            bind:value={selected.region.x}
          /></label
        >
        <label
          >{t('Region Y, %')}<input
            type="number"
            min="0"
            max="100"
            step="0.1"
            bind:value={selected.region.y}
          /></label
        >
        <label
          >{t('Region width, %')}<input
            type="number"
            min="0.1"
            max="100"
            step="0.1"
            bind:value={selected.region.width}
          /></label
        >
        <label
          >{t('Region height, %')}<input
            type="number"
            min="0.1"
            max="100"
            step="0.1"
            bind:value={selected.region.height}
          /></label
        >
        {#if !validCut}<p role="status">
            {t(
              'Check all segments: nonempty intervals within the recording, regions within the frame and total length no greater than 180 seconds.',
            )}
          </p>{/if}
      {/if}
    </fieldset>
    <label
      >{t('Recording name')}<input bind:value={name} maxlength="100" disabled={blocked} /></label
    >
    <label for={limitTitle + '-fit'}>{t('Recording placement')}</label><select
      id={limitTitle + '-fit'}
      bind:value={fit}
      disabled={blocked}
      ><option value="contain">{t('Fit')}</option><option value="cover">{t('Fill and crop')}</option
      ><option value="stretch">{t('Stretch')}</option></select
    >
    <button disabled={blocked || !name.trim() || !validCut} onclick={prepare}
      >{t('Prepare recording for LCD')}</button
    >
  {/if}
  {#if review}
    {#if result?.phase === 'review_ready'}
      <PreparedVideoPreview id={review.id} disabled={blocked || returning} />
      <button disabled={blocked || returning} onclick={save}
        >{t('Save recording to library')}</button
      >
    {:else if result?.phase === 'complete'}<p role="status">
        {t('Recording saved to the library.')}
      </p>
    {:else}<p role="status">
        {t(
          result?.message ??
            (result?.phase === 'failed'
              ? 'Preparation incomplete.'
              : result?.phase === 'cancelled'
                ? 'Preparation cancelled.'
                : 'Preparing or saving recording…'),
        )}
      </p>{/if}
    {#if canReturnToEdit}<button disabled={blocked} onclick={returnToEdit}
        >{t('Return to recording edits')}</button
      >{/if}
  {/if}
  {#if error || status?.message}<p role="alert">{failureText()}</p>{/if}
  {#if capture && !clearAsked}<button
      disabled={closing || result?.phase === 'publishing'}
      onclick={() => (clearAsked = true)}>{t('Clear recording…')}</button
    >{/if}
  {#if clearAsked}<p>{t('Delete the current unsaved recording?')}</p>
    <button disabled={closing} onclick={() => close(true)}
      >{t('Confirm clearing the recording')}</button
    ><button disabled={closing || cleanupRequested} onclick={() => (clearAsked = false)}
      >{t('Keep current recording')}</button
    >{/if}
  <button disabled={closing} onclick={() => close()}>{t('Close screen recording')}</button>
</section>

<style>
  dialog {
    max-width: 440px;
    color: inherit;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    padding: 20px;
  }
  dialog::backdrop {
    background: #0009;
  }
  section {
    border: 1px solid var(--border);
    padding: 16px;
    border-radius: 12px;
    margin: 12px 0;
  }
  label {
    display: grid;
    gap: 6px;
    margin: 12px 0;
  }
  button {
    margin: 4px 8px 4px 0;
  }
</style>
