<script lang="ts">
  import { ValidationError } from './validation-error';
  import {t} from './i18n.svelte';
  import { tick } from 'svelte';
  import ProfileArchive from './ProfileArchive.svelte';
  import { rpc, message, profileGroup, type ProfileContent, type ProfileGroup, type ProfileLibrary, type ProfileEdit } from './api';
  let { refreshEpoch = 0, group, connected, disabled = false, resetSensor = '', mediaKey = '', draft, onload }: {
    refreshEpoch?: number; group: ProfileGroup; connected: boolean; disabled?: boolean; resetSensor?: string; mediaKey?: string;
    draft: () => ProfileContent; onload: (content: ProfileContent) => void;
  } = $props();
  let library = $state<ProfileLibrary | null>(null), slot = $state(0), name = $state('');
  let needsRefresh = $state(false);
  let confirmGroupReset = $state(false);
  let refreshButton: HTMLButtonElement | undefined = $state();
  let groupResetButton: HTMLButtonElement | undefined = $state();
  $effect(() => { void group; void connected; void resetSensor; confirmGroupReset = false; });
  let archiveBusy=$state(false);
  type Failure = { operation: 'refresh' | 'edit'; detail: string };
  let busy = $state(false), error = $state<Failure | ValidationError | null>(null), notice = $state('');
  function errorText(failure: Failure | ValidationError): string {
    if (failure instanceof ValidationError) return t(failure.resource, ...failure.values);
    return failure.operation === 'refresh'
      ? t('Не удалось загрузить список профилей: {0}', failure.detail)
      : t('Изменение профилей не подтверждено: {0}. Проверьте библиотеку перед повторной записью.', failure.detail);
  }
  let selected = $derived(library?.profiles.find(p => p.slot === slot && profileGroup(p.content) === group));
  let unavailable = $derived(busy || archiveBusy || disabled || !connected || needsRefresh);
  let generation = 0;
  $effect(() => {
    void refreshEpoch; void group; void connected;
    generation++; library = null; needsRefresh = false; error = null; notice = '';
    return () => { generation++; };
  });
  async function refresh() {
    const started = generation;
    busy = true; error = null; notice = ''; confirmGroupReset = false;
    try { const result = await rpc<ProfileLibrary>('list_profiles'); if (started === generation) { library = result; needsRefresh = false; } }
    catch (e) { if (started === generation) { error = {operation:'refresh',detail:message(e)}; library = null; } }
    finally { busy = false; }
  }
  async function edit(action: 'save' | 'save_media' | 'rename' | 'delete' | 'reset' | 'reset_group') {
    if (!library || unavailable) return;
    const started = generation;
    busy = true; error = null; notice = '';
    try {
      let change: ProfileEdit;
      if (action === 'reset_group') change = { action, group, sensor_id: group === 'screen' ? null : resetSensor };
      else if (action === 'reset') change = { action, group, slot, sensor_id: group === 'screen' ? null : resetSensor };
      else if (action === 'save' || action === 'save_media') {
        const content: ProfileContent = action === 'save_media' ? {kind:'screen_media', value:mediaKey} : draft();
        if (profileGroup(content) !== group) throw new ValidationError('Канал профиля изменился; повторите сохранение');
        change = { action: 'save', profile: { slot, name, content } };
      } else if (action === 'rename') change = { action, group, slot, name };
      else change = { action, group, slot };
      const result = await rpc<ProfileLibrary>('edit_profile', { expected_revision: library.revision, edit: change });
      if (started !== generation) return;
      library = result;
      notice = 'Библиотека сохранена. Настройки устройства не изменены.';
    } catch (e) {
      if (started === generation) {
        error = e instanceof ValidationError ? e : {operation:'edit',detail:message(e)};
        // A failed reply does not prove the mutation failed. Require readback.
        needsRefresh = true;
      }
    }
    finally {
      busy = false;
      if (action === 'reset_group' && started === generation) {
        confirmGroupReset = false;
        await tick();
        if (started !== generation) return;
        const target = needsRefresh ? refreshButton : groupResetButton;
        if (target?.isConnected) target.focus();
      }
    }
  }
</script>
<details class="profiles">
  <summary>{t("Сохранённые профили")}</summary>
  <p>{t("Загрузите профиль в редактор, затем примените его обычной кнопкой.")}</p>
  <button bind:this={refreshButton} disabled={busy || disabled || !connected} onclick={refresh}>{t("Обновить список профилей")}</button>
  {#if library}
    <label>{t("Слот профиля")}<select bind:value={slot} disabled={unavailable} onchange={event => { const next = Number(event.currentTarget.value); name = library?.profiles.find(p => p.slot === next && profileGroup(p.content) === group)?.name ?? ''; notice = ''; }}>
      {#each Array.from({ length: group === 'screen' ? 8 : 4 }, (_, i) => i) as index}
        <option value={index}>{index + 1} · {library.profiles.find(p => p.slot === index && profileGroup(p.content) === group)?.name ?? t('Пусто')}</option>
      {/each}
    </select></label>
    <label>{t("Имя профиля")}<input bind:value={name} maxlength="128" disabled={unavailable}/></label>
    <div class="buttons">
      <button disabled={unavailable || !selected} onclick={() => { if (selected) { onload(structuredClone($state.snapshot(selected.content))); name = selected.name; notice = 'Профиль загружен в редактор. Применение — отдельное действие.'; } }}>{t("Загрузить профиль в редактор")}</button>
      <button disabled={unavailable || !name.trim()} onclick={() => edit('save')}>{selected ? "" + t("Заменить профиль черновиком") + "" : "" + t("Сохранить черновик в слот") + ""}</button>
      {#if group === 'screen' && mediaKey}<button disabled={unavailable || !name.trim()} onclick={() => edit('save_media')}>{selected ? "" + t("Заменить профиль выбранным роликом") + "" : "" + t("Сохранить выбранный ролик в слот") + ""}</button>{/if}
      <button disabled={unavailable || !selected || !name.trim()} onclick={() => edit('rename')}>{t("Переименовать профиль")}</button>
      <button disabled={unavailable || !selected} onclick={() => edit('delete')}>{t("Удалить профиль")}</button>
      <button disabled={unavailable || (group !== 'screen' && !resetSensor)} onclick={() => edit('reset')}>{t("Сбросить слот к базовым настройкам")}</button>
      <button bind:this={groupResetButton} disabled={unavailable || (group !== 'screen' && !resetSensor)} onclick={() => { confirmGroupReset = true; }}>{t("Сбросить все слоты группы…")}</button>
    </div>
    {#if confirmGroupReset}
      <div role="group" aria-label="{t("Подтверждение сброса группы")}">
        <p>{t("Будут заменены имена и содержимое всех")} {group === 'screen' ? 8 : 4} {t("слотов")} {group === 'screen' ? "" + t("экрана") + "" : group === 'fan' ? "" + t("вентиляторов") + "" : "" + t("помпы") + ""}. {group === 'screen' ? "" + t("В каждом слоте будет базовая тема с часами.") + "" : t("В каждом слоте будет кривая Quiet с датчиком {0}.", resetSensor)} {t("Активные настройки устройства, другие группы и файлы библиотеки сохранятся.")}</p>
        <button disabled={unavailable || (group !== 'screen' && !resetSensor)} onclick={() => edit('reset_group')}>{t("Подтвердить сброс всех слотов")}</button>
        <button disabled={busy} onclick={() => { confirmGroupReset = false; groupResetButton?.focus(); }}>{t("Отмена сброса группы")}</button>
      </div>
    {/if}
  {/if}
  {#if library}<p>{group === 'screen' ? "" + t("Сброс заменит имя и содержимое слота базовой темой с часами.") + "" : "" + t("Сброс заменит слот кривой Quiet с выбранным в редакторе датчиком.") + ""} {t("Для устройства изменения вступят в силу только после загрузки и применения.")}</p>{/if}
  {#if library}<ProfileArchive {group} {slot} revision={library.revision} name={selected?.name??name} hasProfile={!!selected} {connected} disabled={busy||disabled||needsRefresh} bind:busy={archiveBusy}
    onrequireRefresh={()=>needsRefresh=true}
    onlibrary={(result,target)=>{if(result.revision<(library?.revision??0))return;library=result;needsRefresh=false;if(target.group===group&&target.slot===slot)name=result.profiles.find(p=>p.slot===slot&&profileGroup(p.content)===group)?.name??name;}}/>{/if}
  {#if error}<p role="alert">{errorText(error)} {t("Обновите список перед повтором.")}</p>{/if}
  {#if notice}<p role="status">{t(notice)}</p>{/if}
</details>
<style>
  .profiles { border:1px solid var(--border); border-radius:12px; padding:14px; margin:16px 0; }
  summary { cursor:pointer; font-weight:600; }
  label { display:grid; gap:6px; margin:12px 0; }
  .buttons { display:flex; flex-wrap:wrap; gap:8px; }
</style>
