/** Read-only information, not firmware image validation or update authorization. */
export interface FirmwareInfo {
  version: string;
  raw_token: string;
  build_datetime: string | null;
}
const text = (value: unknown, max: number): value is string =>
  typeof value === 'string' &&
  value.length <= max &&
  value.trim().length > 0 &&
  !/[\u0000-\u001f\u007f]/.test(value);
export function firmwareInfo(value: unknown): FirmwareInfo | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
  if (
    !('version' in value) ||
    !text(value.version, 128) ||
    !('raw_token' in value) ||
    !text(value.raw_token, 1024)
  )
    return null;
  const date = 'build_datetime' in value ? value.build_datetime : null;
  if (date !== undefined && date !== null && date !== '' && !text(date, 256)) return null;
  return {
    version: value.version,
    raw_token: value.raw_token,
    build_datetime: typeof date === 'string' && date !== '' ? date : null,
  };
}
