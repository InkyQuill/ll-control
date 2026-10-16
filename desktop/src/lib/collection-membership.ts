import { rpc, type ScreenCollections, type ScreenCollectionsState, type CollectionContext } from './api';

export async function readCollectionMembership(device: string): Promise<ScreenCollections | null> {
  const state = await rpc<ScreenCollectionsState>('get_screen_collections', { dev: device });
  if (!state || !('collections' in state) || !state.persistence) throw new Error('Не удалось прочитать состав коллекций');
  if (state.persistence.pending) throw new Error(`Коллекции ожидают сохранения. ${state.persistence.last_error ?? 'Повторите чтение.'}`);
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
}

/** Snapshot before import so a changed collection cannot be silently overwritten. */
export async function imageReplacementSnapshot(device: string, key: string, context: CollectionContext): Promise<ScreenCollections> {
  let current = await readCollectionMembership(device);
  if (!current) {
    await rpc('migrate_screen_collections', {dev:device});
    current = await readCollectionMembership(device);
  }
  if (!current || !imageInCollection(current,key,context)) throw new Error('Изображение больше не входит в выбранную коллекцию. Обновите библиотеку.');
  return current;
}
export async function replaceCollectionImage(device: string, current: ScreenCollections, context: CollectionContext, oldKey: string, newKey: string): Promise<void> {
  const next = {...current, revision:current.revision+1};
  if (context === 'background') next.backgrounds = current.backgrounds.map(item => item.kind === 'image' && item.key === oldKey ? {...item,key:newKey} : item);
  else if (context === 'image') next.images = current.images.map(key => key === oldKey ? newKey : key);
  else next.watermarks = current.watermarks.map(key => key === oldKey ? newKey : key);
  await rpc('set_screen_collections',{dev:device,expected_revision:current.revision,collections:next});
  const readback = await readCollectionMembership(device);
  if (!readback || readback.revision < next.revision || !imageInCollection(readback,newKey,context) || imageInCollection(readback,oldKey,context)) throw new Error('Замена требует проверки состава коллекций.');
}

export async function videoReplacementSnapshot(device:string,key:string):Promise<ScreenCollections> {
  let current=await readCollectionMembership(device);
  if(!current) {await rpc('migrate_screen_collections',{dev:device});current=await readCollectionMembership(device);}
  if(!current || !videoInCollection(current,key))throw new Error('Ролик больше не входит в коллекцию. Обновите библиотеку.');
  return structuredClone(current);
}
export async function replaceCollectionVideo(device:string,current:ScreenCollections,oldKey:string,newKey:string):Promise<void> {
  const next={...current,revision:current.revision+1,backgrounds:current.backgrounds.map(item=>item.kind==='video' && item.key===oldKey ? {...item,key:newKey}:item)};
  await rpc('set_screen_collections',{dev:device,expected_revision:current.revision,collections:next});
  const readback=await readCollectionMembership(device);
  if(!readback || readback.revision<next.revision || !videoInCollection(readback,newKey) || videoInCollection(readback,oldKey))throw new Error('Замена требует проверки состава коллекций.');
}
