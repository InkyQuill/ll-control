<script lang="ts">
  import { ValidationError, type UiDiagnostic } from './validation-error';
  import {t, diagnosticText} from './i18n.svelte';
  import { untrack, tick } from 'svelte';
  import { videoReplacementSnapshot, replaceCollectionVideo, addImportedToCollection, readCollectionMembership, videoInCollection } from './collection-membership';
  import GalleryVideoEditor from './GalleryVideoEditor.svelte';
  import MediaImport from './MediaImport.svelte';
  import ScreenRecorder from './ScreenRecorder.svelte';
  let captureDevice=$state<string|null>(null);
  $effect(()=>{if(captureDevice && captureDevice!==device)captureDevice=null;});
  import MediaGallery from './MediaGallery.svelte';
  import MediaThumbnail from './MediaThumbnail.svelte';
  import { rpc, message, type PreparedClipInfo, type ScreenCollections } from './api';
  type Diagnostic={detail:UiDiagnostic};
  type ImportFailure=Diagnostic&{context:'edit'|'membership'|'replacement'};
  function importNoticeText(value:string|ImportFailure):string {
    if(typeof value==='string')return t(value);
    switch(value.context){
      case 'edit':return t('Не удалось открыть редактор ролика: {0}',diagnosticText(value.detail));
      case 'membership':return t('Ролик сохранён в каталоге, но добавление в коллекцию не подтверждено: {0}. Проверьте коллекции.',diagnosticText(value.detail));
      case 'replacement':return t('Новый ролик сохранён в каталоге, но замена в коллекции не подтверждена: {0}. Обновите коллекции; автоматического повтора не будет.',diagnosticText(value.detail));
    }
  }
  let { device, collectionEpoch = 0, connected, available, disabled = false, keep = [], onselect, onstop, onbackground, onchoice = () => {}, ondeleted = () => {} }: {
    device: string; collectionEpoch?: number; connected: boolean; available: boolean; disabled?: boolean; keep?: string[];
    onbackground?: (key: string) => void;
    onchoice?: (key: string) => void;
    ondeleted?: (key: string) => void;
    onselect: (key: string) => Promise<void>; onstop: () => Promise<void>;
  } = $props();
  let importBusy=$state(false),editBusy=$state(false);
  let editing=$state<{device:string;key:string;name:string;snapshot:ScreenCollections}|null>(null);
  let editButton=$state<HTMLButtonElement>();
  let restoreEditFocus=$state(false);
  async function beginEdit() {
    if(captureDevice || importBusy || editBusy || editing || builtinChoice || !selectable || !ready || disabled)return;
    const target=device,key=choice,name=clips.find(c=>c.key===key)?.name ?? t('Ролик'),epoch=generation;
    editBusy=true;importNotice='';
    try {const snapshot=await videoReplacementSnapshot(target,key);if(epoch===generation && target===device)editing={device:target,key,name,snapshot};}
    catch(e){if(epoch===generation)importNotice={context:'edit',detail:e instanceof ValidationError ? e : message(e)};}finally{editBusy=false;}
  }
  async function closeEditor() {editing=null;await tick();restoreEditFocus=true;}
  $effect(()=>{
    // Closing mounts the import panel, whose status readback can refresh the
    // catalog. Wait until the edit action is actually enabled before focusing.
    const enabled=!disabled && !importBusy && !editBusy && !editing && !deleting
      && !loading && ready && selectable && !!choice && !builtinChoice;
    if(!restoreEditFocus || !editButton)return;
    if(document.activeElement!==document.body && document.activeElement!==editButton) {
      restoreEditFocus=false;return;
    }
    if(enabled) {editButton.focus();restoreEditFocus=false;}
  });
  async function saveEdit(key:string) {
    const edit=editing,epoch=generation;if(!edit || editBusy)return;
    editBusy=true;
    try {
      await replaceCollectionVideo(edit.device,edit.snapshot,edit.key,key);
      if(epoch===generation && edit.device===device) {await refresh(epoch);if(epoch===generation){choice=key;importNotice='Ролик заменён в коллекции. Исходный файл сохранён для других ссылок.';}}
    }catch(e){if(epoch===generation && edit.device===device)importNotice={context:'replacement',detail:e instanceof ValidationError ? e : message(e)};}
    finally{editBusy=false;if(editing===edit)await closeEditor();}
  }
  $effect(()=>{if(editing && editing.device!==device)editing=null;});
  let importNotice = $state<string|ImportFailure>('');
  async function imported(key: string | null, target: string | null) {
    // A freshly mounted import panel can report the completed gallery job we
    // just loaded. Its unowned notification does not change this catalog.
    if(!target && key && clips.some(clip=>clip.key===key))return;
    const epoch = generation;
    try { if (key && target) await addImportedToCollection(target, 'video', key); }
    catch (e) { if (epoch === generation) importNotice = {context:'membership',detail:e instanceof ValidationError ? e : message(e)}; }
    finally { if (epoch === generation) await refresh(epoch); }
  }
  let gallery = $state(false), catalogMode = $state(false);
  let membership = $state<ScreenCollections | null>(null);
  let open = $state(false), clips = $state<PreparedClipInfo[]>([]);
  let selected = $state<string | null>(null), choice = $state('');
  const selectable = $derived(videoInCollection(membership, choice));
  let loading = $state(false), ready = $state(false), error = $state<UiDiagnostic>('');
  let confirmDelete = $state(''), deleting = $state(false), deleteNotice = $state<string|Diagnostic>('');
  let clipSelect: HTMLSelectElement | undefined = $state();
  let refreshButton: HTMLButtonElement | undefined = $state();
  let generation = 0, request = 0;
  $effect(() => { const key = choice; untrack(() => onchoice(key)); });
  const retainedChoice = $derived(keep.includes(choice));
  $effect(()=>{if(confirmDelete && keep.includes(confirmDelete))confirmDelete='';});
  const builtinChoice = $derived(clips.find(clip => clip.key === choice)?.is_default === true);
  const selectedName = $derived(clips.find(clip => clip.key === selected)?.name);
  async function refresh(epoch = generation) {
    const current = ++request, target = device, canReadSelection = available;
    loading = true; ready = false;
    try {
      const [catalog, active, collections] = await Promise.all([
        rpc<PreparedClipInfo[]>('list_prepared_clips'),
        canReadSelection ? rpc<string | null>('get_lcd_media', {dev: target}) : Promise.resolve(null),
        readCollectionMembership(target)
      ]);
      if (epoch !== generation || current !== request) return;
      membership = collections;
      const items = catalogMode ? catalog : catalog.filter(clip => videoInCollection(collections, clip.key));
      clips = items; selected = active; ready = true; error = '';
      if (!items.some(clip => clip.key === choice)) choice = items.some(clip => clip.key === active) ? active! : items[0]?.key ?? '';
    } catch (e) {
      if (epoch !== generation || current !== request) return;
      error = e instanceof ValidationError ? e : message(e); ready = false; clips = []; selected = null; choice = '';
    } finally {
      if (epoch === generation && current === request) loading = false;
    }
  }
  async function removeClip() {
    if (builtinChoice || retainedChoice || deleting || !connected || disabled || loading || !ready || !!error || !choice || confirmDelete !== choice) return;
    const key = choice, epoch = generation;
    deleting = true; confirmDelete = ''; deleteNotice = '';
    try {
      await rpc<void>('delete_prepared_clip', { key });
      if (epoch === generation) deleteNotice = 'Ролик удалён.';
    } catch (e) {
      if (epoch === generation) deleteNotice = {detail:e instanceof ValidationError ? e : message(e)};
    } finally {
      if (epoch === generation) {
        deleting = false; await refresh(epoch);
        if (epoch === generation && ready && !clips.some(clip => clip.key === key)) ondeleted(key);
        await tick();
        if (epoch === generation) (clipSelect ?? refreshButton)?.focus();
      }
    }
  }
  $effect(() => {
    const online = connected, visible = open;
    void device; void available; void collectionEpoch; void catalogMode;
    const epoch = ++generation;
    clips = []; selected = null; choice = ''; error = ''; loading = false; ready = false;
    deleting = false; confirmDelete = ''; deleteNotice = ''; importNotice = '';
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;
    async function poll() {
      await refresh(epoch);
      if (!stopped) timer = setTimeout(poll, 1500);
    }
    if (online && visible) untrack(() => { void poll(); });
    return () => { stopped = true; generation++; request++; clearTimeout(timer); };
  });
</script>

<details bind:open class="media-library">
  <summary>{t("Библиотека роликов")}</summary>
  <p>{t("Выбранный ролик повторяется и восстанавливается после перезапуска. Применение темы или возврат LocalUI снимает выбор.")}</p>
  {#if !connected}<p role="status">{t("Библиотека недоступна без связи с daemon.")}</p>
  {:else}
    {#if open && !editing && !captureDevice}<MediaImport onbusy={value=>importBusy=value} device={available ? device : null} onimport={(key, target) => { if(target)importNotice = ''; void imported(key, target); }} />{/if}
    {#if open && editing}
      {#key editing.key}<GalleryVideoEditor clipKey={editing.key} name={editing.name} device={editing.device} saving={editBusy} onclose={()=>{if(!editBusy)void closeEditor();}} onsave={key=>void saveEdit(key)}/>{/key}
    {/if}
    {#if open && captureDevice}
      {#key captureDevice}<ScreenRecorder device={captureDevice} onclose={()=>captureDevice=null} onsave={async(key,target)=>{await imported(key,target);}}/>{/key}
    {:else if open && !editing}
      <button disabled={disabled || importBusy || editBusy || !available} onclick={()=>captureDevice=device}>{t("Записать экран…")}</button>
    {/if}
    <button bind:this={refreshButton} disabled={disabled || loading} onclick={() => refresh()}>{t("Обновить ролики")}</button>
    {#if loading && !clips.length}<p role="status">{t("Загрузка роликов…")}</p>{/if}
    {#if error}<p role="alert">{t("Не удалось загрузить ролики:")} {diagnosticText(error)}</p>
    {:else if !loading && !clips.length}<p>{t("Сохранённых роликов пока нет.")}</p>{/if}
    <label><input type="checkbox" bind:checked={catalogMode}/>{t("Показать все файлы видео для управления")}</label>
    {#if clips.length}
      <label>{t("Сохранённый ролик")}
        <select bind:this={clipSelect} bind:value={choice} disabled={disabled || deleting || loading || !ready} onchange={() => { confirmDelete = ''; }}>
          {#each clips as clip (clip.key)}<option value={clip.key}>{clip.name} · {(clip.frames / clip.fps).toFixed(1)} {t("с")}</option>{/each}
        </select>
      </label>
      <label><input type="checkbox" bind:checked={gallery}/>{t("Показать карточки роликов")}</label>
      {#if open && gallery}<MediaGallery {clips} {choice} disabled={disabled || deleting || loading || !ready} onchoose={key => { choice = key; confirmDelete = ''; }}/>{/if}
      {#if open && choice && !gallery}<MediaThumbnail clipKey={choice} name={clips.find(clip => clip.key === choice)?.name ?? t('Ролик')} />{/if}
      {#if !selectable}<p>{t("Файл вне фоновой коллекции. Добавьте его через «Коллекции LCD» для воспроизведения.")}</p>{/if}
      {#if onbackground}<button disabled={!selectable || disabled || deleting || loading || !choice || !!error} onclick={() => onbackground?.(choice)}>{t("Использовать фоном темы")}</button>{/if}
      <button disabled={!selectable || !available || disabled || deleting || loading || !ready || !choice || !!error} onclick={() => onselect(choice)}>{t("Выбрать ролик для LCD")}</button>
      <button bind:this={editButton} disabled={!!captureDevice || disabled || importBusy || editBusy || !!editing || deleting || loading || !ready || !selectable || !choice || builtinChoice} onclick={beginEdit}>{t("Редактировать выбранный ролик")}</button>
      {#if builtinChoice}<p>{t("Встроенный ролик нельзя удалить или редактировать.")}</p>{/if}
      <button disabled={disabled || deleting || loading || !choice || !!error || builtinChoice || retainedChoice || choice === selected} onclick={() => { if(keep.includes(choice))return; confirmDelete = choice; deleteNotice = ''; }}>{t("Удалить ролик…")}</button>
      {#if confirmDelete && confirmDelete === choice}
        <p>{t("Удалить «")}{clips.find(clip => clip.key === confirmDelete)?.name}{t("» из библиотеки? Исходный файл останется на диске.")}</p>
        <button disabled={disabled || deleting || loading || !ready || retainedChoice} onclick={removeClip}>{t("Подтвердить удаление ролика")}</button>
        <button onclick={() => confirmDelete = ''}>{t("Отмена удаления")}</button>
      {/if}
      {#if retainedChoice}<p>{t("Ролик используется в черновике страницы LCD. Замените его фон перед удалением.")}</p>{/if}
      {#if choice === selected}<p>{t("Сначала остановите ролик, чтобы снять сохранённый выбор.")}</p>{/if}
    {/if}
    {#if importNotice}<p role="alert">{importNoticeText(importNotice)}</p>{/if}
    {#if deleteNotice}<p role="status">{typeof deleteNotice==='string'?t(deleteNotice):t('Удаление не подтверждено: {0}. Проверьте обновлённый список перед повтором.',diagnosticText(deleteNotice.detail))}</p>{/if}
    {#if ready && available}<p role="status" aria-live="polite" aria-atomic="true">{selected ? t("Сохранённый выбор: {0}", selectedName ?? t('ролик недоступен в библиотеке')) : "" + t("Ролик для автовосстановления не выбран.") + ""}</p>{/if}
    <button disabled={!available || disabled} onclick={onstop}>{t("Остановить ролик и вернуть LocalUI")}</button>
    {#if !available}<p>{t("Выберите доступное устройство для воспроизведения.")}</p>{/if}
  {/if}
</details>

<style>
  .media-library { padding:16px; border:1px solid var(--border); border-radius:12px; margin:16px 0; }
  summary { cursor:pointer; font-weight:600; }
  label { display:grid; gap:6px; margin:12px 0; }
  select { min-width:0; width:100%; }
  p { overflow-wrap:anywhere; }
  button { margin:4px 8px 4px 0; }
</style>
