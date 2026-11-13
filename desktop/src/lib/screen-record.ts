import type { VideoCrop } from './video-crop';
import { ValidationError } from './validation-error';
import type { SourceMetadata } from './video-range';
export type RecordingPhase = 'reserved' | 'selecting' | 'setup' | 'recording' | 'paused' | 'limit_reached' | 'finalizing' | 'ready' | 'cancelling' | 'cancelled' | 'failed';
export interface RecordingStatus {
  id: string; phase: RecordingPhase; recordedMillis: number; frames: number;
  message: string | null; metadata: SourceMetadata | null; region: VideoCrop | null;
}
export type RecordingAction = 'pause' | 'resume' | 'finish' | {set_region:VideoCrop};
export type RecordingInvoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/** One editor's capture lease; retain until dispose settles. */
export class RecordingSession {
  private opening: Promise<RecordingStatus | null> | null = null;
  private closing: Promise<void> | null = null;
  private disposed = false;
  constructor(readonly id: string, private readonly invoke: RecordingInvoke) {
    if (!/^[0-9a-f]{32}$/.test(id)) throw new ValidationError('Invalid recording ID');
  }
  begin(): Promise<RecordingStatus | null> {
    if (this.disposed) return Promise.reject(new ValidationError('Recording editor is closed'));
    return this.opening ??= this.open();
  }
  private async open(): Promise<RecordingStatus | null> {
    try { await this.invoke<RecordingStatus>('screen_record_begin', { id: this.id }); }
    catch (error) {
      // Recover the same reservation after an ambiguous reply.
      if (!await this.read()) throw error;
    }
    if (this.disposed) return null;
    try { await this.invoke<void>('screen_record_start', { id: this.id }); }
    catch (error) {
      const status = await this.read();
      if (!status || status.phase === 'reserved') throw error;
      return this.disposed ? null : status;
    }
    return this.refresh();
  }
  private async read(): Promise<RecordingStatus | null> {
    const status = await this.invoke<RecordingStatus | null>('screen_record_status');
    return status?.id === this.id ? status : null;
  }
  async refresh(): Promise<RecordingStatus | null> {
    if (this.disposed) return null;
    const status = await this.read();
    return this.disposed ? null : status;
  }
  async control(action: RecordingAction): Promise<void> {
    if (this.disposed) throw new ValidationError('Recording editor is closed');
    // Ambiguous controls are not replayed; caller refreshes status first.
    await this.invoke<void>('screen_record_control', { id: this.id, action });
  }
  async snapshot(): Promise<Uint8Array | null> {
    if (this.disposed) return null;
    const raw = await this.invoke<ArrayBuffer | number[]>('screen_record_snapshot', { id: this.id });
    if (this.disposed) return null;
    const bytes = new Uint8Array(raw);
    if (!bytes.length || bytes.length > 2 * 1024 * 1024) throw new ValidationError('Invalid source preview size');
    return bytes;
  }
  async preview(): Promise<string> {
    if (this.disposed) throw new ValidationError('Recording editor is closed');
    return this.invoke<string>('media_playback_url', { id: this.id, kind: 'recording' });
  }
  dispose(): Promise<void> {
    this.disposed = true;
    if (this.closing) return this.closing;
    // Cancelling before Begin settles can miss the future reservation.
    this.closing = (async () => {
      await this.opening?.catch(() => {});
      if (this.opening) await this.invoke<void>('screen_record_cancel', { id: this.id });
    })();
    // Surface failure and permit an explicit cleanup retry.
    void this.closing.catch(() => { this.closing = null; });
    return this.closing;
  }
}

export interface RecordingEdit {
  name: string;
  fit: 'contain' | 'cover' | 'stretch';
  cut: { segments: { start_ms: number; end_ms: number; crop?: import('./video-crop').VideoCrop }[] } | null;
}
export interface RecordingReviewStatus {
  id: string;
  phase: 'uploading' | 'inspecting' | 'source_ready' | 'encoding' | 'validating' | 'remuxing' | 'review_ready' | 'cancelling' | 'transferring' | 'publishing' | 'complete' | 'cancelled' | 'failed';
  key: string | null;
  message: string | null;
}
/** Separate media lease: disposing capture does not release an admitted review. */
export class RecordingReview {
  private admission: Promise<void> | null = null;
  private preparation: Promise<void> | null = null;
  private closing: Promise<void> | null = null;
  private disposed = false;
  constructor(readonly id: string, readonly recordingId: string, private readonly invoke: RecordingInvoke) {
    if (![id, recordingId].every(value => /^[0-9a-f]{32}$/.test(value))) throw new ValidationError('Invalid recording review ID');
  }
  private async read(): Promise<RecordingReviewStatus | null> {
    const status = await this.invoke<RecordingReviewStatus | null>('media_status');
    return status?.id === this.id ? status : null;
  }
  async refresh(): Promise<RecordingReviewStatus | null> {
    if (this.disposed) return null;
    const status = await this.read();
    return this.disposed ? null : status;
  }
  prepare(edit: RecordingEdit): Promise<void> {
    if (this.disposed) return Promise.reject(new ValidationError('Recording review is closed'));
    if (this.preparation) return this.preparation;
    this.admission = (async () => {
      try { await this.invoke('screen_record_admit', { recordingId: this.recordingId, id: this.id, ...edit }); }
      catch (error) {
        // Read only after synchronous native admission settles; never adopt a foreign ID.
        if (!await this.read()) throw error;
      }
    })();
    this.preparation = (async () => {
      await this.admission;
      if (this.disposed) return;
      // Lost Prepare reply is surfaced, never retried while encoding may continue.
      await this.invoke<void>('screen_record_prepare', { id: this.id });
    })();
    return this.preparation;
  }
  async confirm(): Promise<void> {
    if (this.disposed) throw new ValidationError('Recording review is closed');
    const status = await this.read();
    if (this.disposed || status?.phase !== 'review_ready') throw new ValidationError('Recording is not ready to save');
    // Explicit user action only; recover an ambiguous publication through status.
    await this.invoke<void>('media_confirm', { id: this.id });
  }
  dispose(): Promise<void> {
    this.disposed = true;
    if (this.closing) return this.closing;
    this.closing = (async () => {
      // Wait for admission, not preparation: cancellation must interrupt encoding.
      await this.admission?.catch(() => {});
      if (!this.admission) return;
      const status = await this.read();
      if (!status || ['complete', 'cancelled', 'failed', 'publishing'].includes(status.phase)) return;
      await this.invoke<void>('media_cancel', { id: this.id });
    })();
    void this.closing.catch(() => { this.closing = null; });
    return this.closing;
  }
}
