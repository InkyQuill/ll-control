import type { ProfileGroup, ProfileLibrary, NamedProfile } from './api';
export interface ArchiveReply {
  state: {
    id: string;
    direction: 'import' | 'export';
    group: ProfileGroup;
    slot: number;
    expected_revision: number;
    phase: 'uploading' | 'ready' | 'imported' | 'failed' | 'cancelled';
    size: number;
    received: number;
    profile: NamedProfile | null;
    library: ProfileLibrary | null;
    last_error: string | null;
    committed_revision: number | null;
  };
  offset?: number | null;
  bytes?: number[];
  eof: boolean;
}
export type ArchivePhase =
  | 'choosing'
  | 'preparing'
  | 'packing'
  | 'uploading'
  | 'downloading'
  | 'committing'
  | 'recovering'
  | 'cancelling'
  | 'cancelled'
  | 'complete'
  | 'failed'
  | 'uncertain';
export interface ArchiveStatus {
  id: string;
  mode: 'import' | 'export';
  group: ProfileGroup;
  slot: number;
  phase: ArchivePhase;
  transferred: number;
  total: number;
  file_name: string | null;
  message: string | null;
  message_code?: string | null;
  result: ArchiveReply | null;
}
export interface PendingArchive {
  id: string;
  group: ProfileGroup;
  slot: number;
}
export const pendingArchiveKey = 'lconnect.archive.pending';
export function pendingArchive(value: string | null): PendingArchive | null {
  if (!value) return null;
  try {
    const p = JSON.parse(value) as Partial<PendingArchive>;
    return typeof p.id === 'string' &&
      /^[a-f0-9]{32}$/.test(p.id) &&
      ['screen', 'fan', 'pump'].includes(p.group ?? '') &&
      Number.isInteger(p.slot) &&
      p.slot! >= 0 &&
      p.slot! < (p.group === 'screen' ? 8 : 4)
      ? (p as PendingArchive)
      : null;
  } catch {
    return null;
  }
}
export const profileGroupNames: Record<ProfileGroup, string> = {
  screen: 'Screen',
  fan: 'Fans',
  pump: 'Pump',
};
export const archiveTerminal = (phase: ArchivePhase) =>
  ['cancelled', 'complete', 'failed', 'uncertain'].includes(phase);
export const archivePhase: Record<ArchivePhase, string> = {
  choosing: 'Choosing file',
  preparing: 'Preparing file',
  packing: 'Preparing archive',
  uploading: 'Uploading archive',
  downloading: 'Saving archive',
  committing: 'Committing changes',
  recovering: 'Reading result',
  cancelling: 'Cancelling transfer',
  cancelled: 'Operation cancelled',
  complete: 'Operation completed',
  failed: 'Operation failed',
  uncertain: 'Result needs verification',
};

export function readPendingArchive(): PendingArchive | null {
  try {
    return typeof localStorage === 'undefined'
      ? null
      : pendingArchive(localStorage.getItem(pendingArchiveKey));
  } catch {
    return null;
  }
}
