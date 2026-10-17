import { ValidationError, type UiDiagnostic } from './validation-error';
import { rpc, type ScreenCollections, type ScreenCollectionsState, type CollectionContext } from './api';

/** Only callers with an acknowledged imported asset may construct this outcome.
 * A remote error with the same message/name is still an ordinary diagnostic. */
export class ImportedImageMembershipError extends Error {
  constructor(readonly detail: UiDiagnostic) { super(typeof detail === 'string' ? detail : detail.message); this.name = 'ImportedImageMembershipError'; }
}

export async function readCollectionMembership(device: string): Promise<ScreenCollections | null> {
  const state = await rpc<ScreenCollectionsState>('get_screen_collections', { dev: device });
  const object = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value);
  const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every(item => typeof item === 'string');
  if (!object(state) || !Object.hasOwn(state, 'collections') || !object(state.persistence)
      || typeof state.persistence.pending !== 'boolean'
      || (state.persistence.last_error != null && typeof state.persistence.last_error !== 'string'))
    throw new ValidationError('Не удалось прочитать состав коллекций');
  if (state.persistence.pending) {
    if (state.persistence.last_error) throw new ValidationError('Коллекции ожидают сохранения. {0}', [state.persistence.last_error]);
    throw new ValidationError('Коллекции ожидают сохранения. Повторите чтение.');
  }
  const collections = state.collections;
  if (collections !== null && (!object(collections) || collections.version !== 1
      || !Number.isSafeInteger(collections.revision) || collections.revision < 0
      || !strings(collections.images) || !strings(collections.watermarks)
      || !Array.isArray(collections.backgrounds) || !collections.backgrounds.every(item =>
        object(item) && (item.kind === 'image' || item.kind === 'video') && typeof item.key === 'string')))
    throw new ValidationError('Не удалось прочитать состав коллекций');
  return state.collections;
}
export function imageInCollection(collections: ScreenCollections | null, key: string, context: CollectionContext): boolean {
  if (!collections) return true;
  return context === 'background' ? collections.backgrounds.some(asset => asset.kind === 'image' && asset.key === key)
    : (context === 'image' ? collections.images : collections.watermarks).includes(key);
}
export function videoInCollection(collections: ScreenCollections | null, key: string): boolean {
  return !collections || collections.backgrounds.some(asset => asset.kind === 'video' && asset.key === key);
}

/** Import publication and membership are separate: errors leave the file in the
 * catalog for explicit recovery, never retry a stale whole-collection snapshot. */
export async function addImportedToCollection(device: string, kind: 'image' | 'video', key: string): Promise<void> {
  const current = await readCollectionMembership(device);
  if (!current) return; // Legacy catalog already exposes every imported file.
  if (kind === 'image' ? current.images.includes(key) : videoInCollection(current, key)) return;
  const next = { ...current, revision: current.revision + 1,
    images: kind === 'image' ? [...current.images, key] : current.images,
    backgrounds: kind === 'video' ? [...current.backgrounds, { kind, key }] : current.backgrounds };
  await rpc('set_screen_collections', { dev: device, expected_revision: current.revision, collections: next });
  const readback = await readCollectionMembership(device);
  if (!readback || readback.revision < next.revision
      || !(kind === 'image' ? readback.images.includes(key) : videoInCollection(readback, key)))
    throw new ValidationError('Добавление требует проверки состава коллекций.');
}

/** Snapshot before import so a changed collection cannot be silently overwritten. */
export async function imageReplacementSnapshot(device: string, key: string, context: CollectionContext): Promise<ScreenCollections> {
  let current = await readCollectionMembership(device);
  if (!current) {
    await rpc('migrate_screen_collections', {dev:device});
    current = await readCollectionMembership(device);
  }
  if (!current || !imageInCollection(current,key,context)) throw new ValidationError('Изображение больше не входит в выбранную коллекцию. Обновите библиотеку.');
  return current;
}
export async function replaceCollectionImage(device: string, current: ScreenCollections, context: CollectionContext, oldKey: string, newKey: string): Promise<void> {
  const next = {...current, revision:current.revision+1};
  if (context === 'background') next.backgrounds = current.backgrounds.map(item => item.kind === 'image' && item.key === oldKey ? {...item,key:newKey} : item);
  else if (context === 'image') next.images = current.images.map(key => key === oldKey ? newKey : key);
  else next.watermarks = current.watermarks.map(key => key === oldKey ? newKey : key);
  await rpc('set_screen_collections',{dev:device,expected_revision:current.revision,collections:next});
  const readback = await readCollectionMembership(device);
  if (!readback || readback.revision < next.revision || !imageInCollection(readback,newKey,context) || imageInCollection(readback,oldKey,context)) throw new ValidationError('Замена требует проверки состава коллекций.');
}

export async function videoReplacementSnapshot(device:string,key:string):Promise<ScreenCollections> {
  let current=await readCollectionMembership(device);
  if(!current) {await rpc('migrate_screen_collections',{dev:device});current=await readCollectionMembership(device);}
  if(!current || !videoInCollection(current,key))throw new ValidationError('Ролик больше не входит в коллекцию. Обновите библиотеку.');
  return structuredClone(current);
}
export async function replaceCollectionVideo(device:string,current:ScreenCollections,oldKey:string,newKey:string):Promise<void> {
  const next={...current,revision:current.revision+1,backgrounds:current.backgrounds.map(item=>item.kind==='video' && item.key===oldKey ? {...item,key:newKey}:item)};
  await rpc('set_screen_collections',{dev:device,expected_revision:current.revision,collections:next});
  const readback=await readCollectionMembership(device);
  if(!readback || readback.revision<next.revision || !videoInCollection(readback,newKey) || videoInCollection(readback,oldKey))throw new ValidationError('Замена требует проверки состава коллекций.');
}
