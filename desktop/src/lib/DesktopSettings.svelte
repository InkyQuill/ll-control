<script lang="ts">
  import {t} from './i18n.svelte';
  import {invoke} from '@tauri-apps/api/core';
  import {message} from './api';
  import {ValidationError} from './validation-error';
  interface Preferences {closeToTray:boolean;startMinimized:boolean;autostart:boolean}
  interface Snapshot {revision:number;preferences:Preferences;registration:{present:boolean;enabled:boolean|null;managed:boolean;problem:string|null};integrationError:string|null;trayReady:boolean;trayError:string|null;trayErrorCode?:string|null}
  let open=$state(false),busy=$state(false),snapshot=$state<Snapshot|null>(null),draft=$state<Preferences|null>(null),base=$state<number|null>(null),error=$state<ValidationError|string|null>(null),readError=$state<ValidationError|string|null>(null),notice=$state('');
  let epoch=0;
  const stale=$derived(snapshot!==null&&base!==snapshot.revision);
  const registrationPending=$derived(!!snapshot&&snapshot.registration.enabled!==snapshot.preferences.autostart);
  function valid(value:Snapshot|null):value is Snapshot{return !!value&&typeof value.revision==='number'&&!!value.preferences&&!!value.registration;}
  async function read(reload=false){
    const generation=epoch;
    try {const value=await invoke<Snapshot|null>('desktop_settings_get');if(generation!==epoch||!open)return;if(!valid(value))throw new ValidationError('Настройки приложения недоступны.');snapshot=value;
      if(reload||draft===null){draft={...value.preferences};base=value.revision;}readError=null;if(reload)error=null;
    }catch(e){if(generation===epoch&&open)readError=e instanceof ValidationError?e:message(e);}
  }
  $effect(()=>{
    if(!open)return;
    let stopped=false;let timer:ReturnType<typeof setTimeout>;
    async function poll(){if(!busy)await read();if(!stopped)timer=setTimeout(poll,2000);}
    timer=setTimeout(poll,0);return()=>{stopped=true;clearTimeout(timer);epoch++;};
  });
  async function save(){
    if(busy||!draft||base===null||stale)return;
    epoch++;busy=true;error=null;notice='';
    try {const value=await invoke<Snapshot>('desktop_settings_save',{expectedRevision:base,preferences:$state.snapshot(draft)});if(!valid(value))throw new ValidationError('Ответ сохранения не подтверждён. Обновите состояние.');snapshot=value;draft={...value.preferences};base=value.revision;notice=value.integrationError?'Параметры сохранены. Регистрация автозапуска требует повторной проверки.':'Параметры приложения сохранены.';}
    catch(e){error=e instanceof ValidationError?e:message(e);}finally{busy=false;}
  }
  async function exit(){if(busy)return;busy=true;error=null;try{await invoke('desktop_exit');notice='Завершаем активные операции перед выходом…';}catch(e){error=e instanceof ValidationError?e:message(e);busy=false;}}
</script>
<details ontoggle={event=>open=event.currentTarget.open}>
  <summary>{t("Приложение и автозапуск")}</summary>
  <p>{t("Эти настройки относятся к окну приложения. Daemon продолжает управлять охлаждением и LCD после выхода из GUI.")}</p>
  {#if snapshot&&draft}
    <label><input type="checkbox" bind:checked={draft.closeToTray} disabled={busy}/>{t("Сворачивать в tray при закрытии окна")}</label>
    <label><input type="checkbox" bind:checked={draft.startMinimized} disabled={busy}/>{t("Запускать приложение свёрнутым")}</label>
    <label><input type="checkbox" bind:checked={draft.autostart} disabled={busy}/>{t("Запускать приложение при входе в систему")}</label>
    <p role="status">Tray: {snapshot.trayReady?"" + t("доступен") + "":"" + t("недоступен") + ""}. {snapshot.trayReady?"" + t("Команды «Показать» и «Выйти» доступны в меню значка.") + "":"" + t("Окно не останется скрытым. Закрытие завершит GUI с ожиданием активных операций.") + ""}</p>
    <p role="status">{t("Регистрация автозапуска:")} {snapshot.registration.enabled===null?"" + t("состояние не подтверждено") + "":snapshot.registration.enabled?"" + t("включена") + "":"" + t("выключена") + ""}.</p>
    {#if snapshot.trayErrorCode==='host_unconfirmed'}
      <p>{t('Регистрация приложения в tray host не подтверждена.')}</p>
    {:else}
      {#if snapshot.trayErrorCode==='unavailable'}<p>{t('Не удалось подготовить значок tray.')}</p>{/if}
      {#if snapshot.trayError}<p>{snapshot.trayError}</p>{/if}
    {/if}
    {#if snapshot.registration.problem}<p role="alert">{snapshot.registration.problem}</p>{/if}
    {#if snapshot.integrationError}<p role="alert">{snapshot.integrationError}</p>{:else if registrationPending}<p role="alert">{t("Сохранённое состояние автозапуска не подтверждено регистрацией. После устранения причины повторите регистрацию.")}</p>{/if}
    {#if stale}<p role="alert">{t("Настройки изменились в другом окне. Обновите значения перед сохранением.")}</p>{/if}
    <button disabled={busy||stale} onclick={save}>{snapshot.integrationError||registrationPending?"" + t("Сохранить и повторить регистрацию") + "":"" + t("Сохранить параметры приложения") + ""}</button>
  {/if}
  <button disabled={busy} onclick={()=>read(true)}>{t("Обновить параметры приложения")}</button>
  <button disabled={busy} onclick={exit}>{t("Выйти из приложения полностью")}</button>
  {#if notice}<p role="status">{t(notice)}</p>{/if}
  {#if readError}<p role="alert">{readError instanceof ValidationError?t(readError.resource,...readError.values):readError}</p>{/if}
  {#if error}<p role="alert">{error instanceof ValidationError?t(error.resource,...error.values):error}</p>{/if}
</details>
<style>details{border:1px solid var(--border);border-radius:12px;padding:18px;margin-block:16px}summary{cursor:pointer;font-weight:600}label{display:flex;gap:10px;align-items:center;margin-block:14px}input[type=checkbox]{width:18px;height:18px;flex:none}p{line-height:1.5}button{margin:6px 8px 6px 0}[role=alert]{color:var(--danger)}</style>
