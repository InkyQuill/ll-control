import { ValidationError } from './validation-error';
import { rpc } from './api';
export interface FontChoice {
  key: string;
  name: string;
}
interface UploadState {
  id: string;
  size: number;
  received: number;
  asset: string | null;
  cancelled: boolean;
}
const CHUNK = 64 * 1024;
export async function uploadFont(
  file: File,
  signal: AbortSignal,
  progress: (received: number) => void,
): Promise<string> {
  if (!file.size || file.size > 16 * 1024 * 1024)
    throw new ValidationError('Шрифт должен быть размером от 1 байта до 16 MiB');
  let id: string | undefined;
  const call = (request: object) => rpc<UploadState>('font_upload', { request });
  try {
    signal.throwIfAborted();
    const started = await call({ action: 'begin', size: file.size });
    id = started.id;
    signal.throwIfAborted();
    for (let offset = 0; offset < file.size; offset += CHUNK) {
      signal.throwIfAborted();
      const bytes = Array.from(
        new Uint8Array(await file.slice(offset, offset + CHUNK).arrayBuffer()),
      );
      signal.throwIfAborted();
      const state = await call({ action: 'chunk', id, offset, bytes });
      if (state.received !== offset + bytes.length)
        throw new ValidationError('Неверное смещение загрузки шрифта');
      progress(state.received);
    }
    signal.throwIfAborted();
    let finished: UploadState;
    try {
      finished = await call({ action: 'finish', id });
    } catch (error) {
      signal.throwIfAborted();
      // Read back an ambiguous finish; never start a second upload or duplicate the file.
      finished = await call({ action: 'status', id });
      if (!finished.asset) throw error;
    }
    signal.throwIfAborted();
    if (!finished.asset || !/^[0-9a-f]{32}$/.test(finished.asset))
      throw new ValidationError('Шрифт не сохранён');
    return finished.asset;
  } finally {
    // Cancel releases the session, including completed uploads, without deleting the asset.
    if (id) await call({ action: 'cancel', id }).catch(() => {});
  }
}
