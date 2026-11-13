/** Saved artifacts only: never update authorization, live progress or success. */
export type RecoveryState =
  'no_saved_operation' | 'journal_only' | 'guarded' | 'guard_only' | 'inconsistent' | 'unreadable';
export type SavedStage =
  'armed' | 'effects_possible' | 'not_started' | 'unknown' | 'awaiting_reconnect';
export interface RecoveryGuard {
  settings_key: string;
  operation_id: string;
  image_sha256: string;
}
export interface SavedProgress {
  total_chunks: number;
  chunk_writes: number;
  chunk_echoes: number;
  pump_hold_written: boolean;
  fan_hold_written: boolean;
  activation_written: boolean;
}
export interface SavedOperation extends RecoveryGuard {
  stage: SavedStage;
  vid: number;
  pid: number;
  serial: string | null;
  previous_hid_path: string;
  progress: SavedProgress | null;
  diagnostic: string | null;
}
export interface FirmwareRecovery {
  state: RecoveryState;
  operation: SavedOperation | null;
  guards: RecoveryGuard[];
  diagnostic: string | null;
}
const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
const text = (value: unknown, max: number, diagnostic = false): value is string =>
  typeof value === 'string' &&
  new TextEncoder().encode(value).length <= max &&
  (diagnostic || (value.length > 0 && !/\p{Cc}/u.test(value)));
const optionalText = (value: unknown, max: number, diagnostic = false): value is string | null =>
  value === null || text(value, max, diagnostic);
const uint = (value: unknown, max = 0xffffffff): value is number =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= max;
function guard(value: unknown): value is RecoveryGuard {
  return (
    object(value) &&
    text(value.settings_key, 4096) &&
    typeof value.operation_id === 'string' &&
    /^[\da-f]{32}$/i.test(value.operation_id) &&
    typeof value.image_sha256 === 'string' &&
    /^[\da-f]{64}$/i.test(value.image_sha256)
  );
}
function progress(value: unknown): value is SavedProgress | null {
  return (
    value === null ||
    (object(value) &&
      uint(value.total_chunks) &&
      uint(value.chunk_writes) &&
      uint(value.chunk_echoes) &&
      value.chunk_echoes <= value.chunk_writes &&
      value.chunk_writes <= value.total_chunks &&
      typeof value.pump_hold_written === 'boolean' &&
      typeof value.fan_hold_written === 'boolean' &&
      typeof value.activation_written === 'boolean')
  );
}
function operation(value: unknown): value is SavedOperation {
  if (!object(value) || !guard(value)) return false;
  const row = value as unknown as Record<string, unknown>;
  return (
    typeof row.stage === 'string' &&
    ['armed', 'effects_possible', 'not_started', 'unknown', 'awaiting_reconnect'].includes(
      String(row.stage),
    ) &&
    uint(row.vid, 65535) &&
    uint(row.pid, 65535) &&
    optionalText(row.serial, 256) &&
    text(row.previous_hid_path, 4096) &&
    progress(row.progress) &&
    optionalText(row.diagnostic, 8192, true)
  );
}
export function firmwareRecovery(value: unknown): FirmwareRecovery | null {
  if (
    !object(value) ||
    typeof value.state !== 'string' ||
    ![
      'no_saved_operation',
      'journal_only',
      'guarded',
      'guard_only',
      'inconsistent',
      'unreadable',
    ].includes(String(value.state)) ||
    !optionalText(value.diagnostic, 8192, true) ||
    !Array.isArray(value.guards) ||
    value.guards.length > 256 ||
    !value.guards.every(guard) ||
    (value.operation !== null && !operation(value.operation))
  )
    return null;
  const op = value.operation as SavedOperation | null,
    guards = value.guards as RecoveryGuard[],
    state = value.state as RecoveryState;
  const match =
    op !== null &&
    guards.length === 1 &&
    guards[0].operation_id === op.operation_id &&
    guards[0].image_sha256 === op.image_sha256 &&
    guards[0].settings_key === op.settings_key;
  const inferred: RecoveryState =
    op === null
      ? guards.length
        ? 'guard_only'
        : 'no_saved_operation'
      : match
        ? 'guarded'
        : !guards.length && ['armed', 'not_started'].includes(op.stage)
          ? 'journal_only'
          : 'inconsistent';
  if (state === 'unreadable' ? op !== null || guards.length !== 0 : state !== inferred) return null;
  return { state, operation: op, guards, diagnostic: value.diagnostic };
}
