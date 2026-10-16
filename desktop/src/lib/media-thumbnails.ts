import { invoke } from '@tauri-apps/api/core';
import { message } from './api';

type Job = {
  id: string; key: string; signal: AbortSignal;
  resolve: (bytes: number[]) => void; reject: (error: unknown) => void;
  abort: () => void;
};
const queue: Job[] = [];
let active: Job | undefined;
const cancelled = () => new DOMException('Thumbnail cancelled', 'AbortError');

// One native decoder for all visible cards and profile previews. Aborting an
// active request rejects its consumer immediately, but admission stays occupied
// until native decoding has acknowledged cancellation/reaped its child.
async function pump() {
  if (active) return;
  const job = queue.shift();
  if (!job) return;
  active = job;
  try {
    for (let attempt = 0; attempt < 6; attempt++) {
      if (job.signal.aborted) throw cancelled();
      try {
        const bytes = await invoke<number[]>('media_thumbnail', {id: job.id, key: job.key});
        if (!job.signal.aborted) job.resolve(bytes);
        break;
      } catch (e) {
        if (message(e) !== 'Thumbnail worker busy' || attempt === 5) throw e;
        await new Promise(resolve => setTimeout(resolve, 150));
      }
    }
  } catch (e) { job.reject(e); }
  finally {
    job.signal.removeEventListener('abort', job.abort);
    active = undefined;
    void pump();
  }
}

export function requestThumbnail(key: string, signal: AbortSignal): Promise<number[]> {
  if (signal.aborted) return Promise.reject(cancelled());
  if (queue.length + Number(!!active) >= 32) return Promise.reject(new Error('Очередь миниатюр заполнена. Повторите позже.'));
  return new Promise((resolve, reject) => {
    const job: Job = {id: crypto.randomUUID().replaceAll('-', ''), key, signal, resolve, reject, abort: () => {}};
    job.abort = () => {
      const index = queue.indexOf(job);
      if (index >= 0) {
        queue.splice(index, 1);
        signal.removeEventListener('abort', job.abort);
      }
      if (active === job) void invoke('media_thumbnail_cancel', {id: job.id}).catch(() => {});
      reject(cancelled());
    };
    signal.addEventListener('abort', job.abort, {once: true});
    queue.push(job);
    void pump();
  });
}
