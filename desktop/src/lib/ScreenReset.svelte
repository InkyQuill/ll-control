<script lang="ts">
  import StudioPopover from "./StudioPopover.svelte";
  import {t} from './i18n.svelte';
  import { tick, untrack } from 'svelte';
  import { rpc, message, type ScreenResetReadback, type ProfileLibrary, type ScreenCollectionsState } from './api';
  let { device, connected, active = true, disabled = false, onbusy = () => {}, onchange = () => {} }: {
    device: string; connected: boolean; active?: boolean; disabled?: boolean;
    onbusy?: (busy: boolean) => void; onchange?: (device: string) => void;
  } = $props();
  type ResetRequest = { dev: string; operation_id: string; expected_profile_revision: number; expected_collection_revision: number };
  let open = $state(false), busy = $state(false), loading = $state(false), ready = $state(false), confirm = $state(false);
  let readback = $state<ScreenResetReadback | null>(null), profileRevision = $state(0), collectionRevision = $state(0);
  type Failure = { kind: 'rejected' | 'unconfirmed'; diagnostic: string };
  let failure = $state<Failure | null>(null);
  let pending = $state<ResetRequest | null>(null), error = $state(''), notice = $state('');
  let refreshButton: HTMLButtonElement | undefined = $state();
  let generation = 0, request = 0;
  const locked = $derived(!connected || disabled || busy || loading || !ready || !!readback?.persistence.pending);
  const unresolved = $derived(pending?.dev === device);
  async function refresh(epoch = generation) {
    const seq = ++request, target = device;
    loading = true; ready = false; confirm = false;
    try {
      const [read, profiles, collections] = await Promise.all([
        rpc<ScreenResetReadback>('get_screen_reset', { dev: target }),
        rpc<ProfileLibrary>('list_profiles'),
        rpc<ScreenCollectionsState>('get_screen_collections', { dev: target })
      ]);
      if (epoch !== generation || seq !== request) return;
      readback = read; profileRevision = profiles.revision; collectionRevision = collections.collections?.revision ?? 0;
      ready = !read.persistence.pending && !collections.persistence.pending; error = '';
      if (pending?.dev === target && read.receipt?.operation_id === pending.operation_id) {
        pending = null;
        failure = null;
        onchange(target);
        notice = read.lcd.last_error ? 'Сброс сохранён. Передача на LCD завершилась ошибкой.' : 'Сброс сохранён, базовая тема отправлена на LCD.';
      }
    } catch (e) { if (epoch === generation && seq === request) error = message(e); }
    finally { if (epoch === generation && seq === request) loading = false; }
  }
  $effect(() => {
    void device; void connected; void open; void active;
    const epoch = ++generation;
    readback = null; ready = false; loading = false; confirm = false; error = ''; notice = ''; failure = null;
    if (open && active && connected) untrack(() => void refresh(epoch));
  });
  async function mutate(reapply = false) {
    if (locked || (pending && pending.dev !== device)) return;
    if (!reapply && !pending && !confirm) return;
    const target = device, epoch = generation;
    const oldState = readback;
    if (reapply && (!oldState?.receipt || !oldState.reapply_available)) return;
    const reset = pending ?? { dev: target, operation_id: crypto.randomUUID().replaceAll('-', ''), expected_profile_revision: profileRevision, expected_collection_revision: collectionRevision };
    if (!reapply) pending = reset;
    busy = true; onbusy(true); ready = false; confirm = false; error = ''; notice = ''; failure = null;
    let outcome: Failure | null = null;
    try {
      if (reapply) await rpc('reapply_screen_reset', { dev: target, operation_id: oldState!.receipt!.operation_id, expected_generation: oldState!.lcd.generation });
      else await rpc('reset_screen', $state.snapshot(reset));
    } catch (e) {
      if (!reapply && typeof e === 'object' && e !== null && 'code' in e && e.code === -32002) {
        pending = null;
        outcome = { kind: 'rejected', diagnostic: message(e) };
      } else outcome = { kind: 'unconfirmed', diagnostic: message(e) };
    } finally {
      if (connected && open && active) await refresh(generation);
      busy = false; onbusy(false); onchange(target);
      if (epoch === generation) {
        failure = outcome;
        await tick();
        if (epoch === generation && active && open) refreshButton?.focus();
      }
    }
  }
</script>

<StudioPopover label={t("Сброс экрана LCD")} bind:open icon="rotate" text>
  <p>{t("Восстановить базовую тему, яркость 50%, поворот 0°, встроенный состав коллекций этого устройства и все восемь общих Screen-слотов. Общие слоты изменятся для всех устройств.")}</p>
  <p>{t("Файлы изображений, видео и шрифтов, охлаждение и подсветка сохранятся. Локальный черновик темы не изменится; базовую тему можно загрузить кнопкой «Загрузить активную тему».")}</p>
  <button bind:this={refreshButton} disabled={!connected || busy || loading} onclick={() => refresh()}>{t("Проверить результат сброса")}</button>
  {#if loading}<p role="status">{t("Чтение состояния сброса…")}</p>{/if}
  {#if error}<p role="alert">{t('Не удалось проверить сброс:')} <span>{error}</span></p>{/if}
  {#if notice}<p role="status">{t(notice)}</p>{/if}
  {#if failure}<p role="status">{failure.kind === 'rejected' ? t('Сброс отклонён:') : t('Ответ не подтверждён:')} <span>{failure.diagnostic}</span>. {failure.kind === 'rejected' ? t('Проверьте состояние перед новым подтверждением.') : t('Проверьте результат.')}</p>{/if}
  {#if readback?.persistence.pending}<p role="status">{t("Настройки ожидают записи. Обновите состояние.")}</p>{/if}
  {#if readback?.lcd.last_error}<p role="alert">{t("Ошибка LCD:")} <span>{readback.lcd.last_error}</span></p>{/if}
  {#if unresolved}
    <p role="status">{t("Результат запроса ещё не подтверждён. Повтор использует ту же операцию и не сбросит настройки второй раз.")}</p>
    <button disabled={locked} onclick={() => mutate()}>{t("Повторить запрос сброса")}</button>
  {:else if pending}
    <p role="status">{t("Есть неподтверждённый сброс другого устройства. Вернитесь к нему для проверки.")}</p>
  {:else if confirm}
    <p>{t("Сбросить экран и восемь общих слотов?")}</p>
    <button disabled={locked} onclick={() => mutate()}>{t("Подтвердить сброс экрана")}</button>
    <button disabled={busy} onclick={() => { confirm = false; refreshButton?.focus(); }}>{t("Отмена сброса")}</button>
  {:else}
    <button disabled={locked} onclick={() => confirm = true}>{t("Сбросить экран…")}</button>
  {/if}
  {#if readback?.reapply_available}
    <p>{t("Сброс сохранён. Можно повторить только отправку базовой темы на LCD.")}</p>
    <button disabled={locked || !!pending} onclick={() => mutate(true)}>{t("Повторно применить к LCD")}</button>
  {/if}
</StudioPopover>
<style>


  button { margin: .2rem; }
</style>
