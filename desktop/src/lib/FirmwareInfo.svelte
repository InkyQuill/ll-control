<script lang="ts">
  import {onDestroy} from 'svelte';
  import {t} from './i18n.svelte';
  import {rpc,message} from './api';
  import {firmwareInfo,type FirmwareInfo} from './firmware-info';
  // The parent keys this resource by selection/availability epoch. Leaving the
  // view invalidates UI ownership; the daemon finishes its indivisible HID read.
  let {device,connected}:{device:string;connected:boolean}=$props();
  let snapshot=$state<FirmwareInfo|null>(null),busy=$state(false);
  let failure=$state<'read'|'invalid'|null>(null),diagnostic=$state('');
  let sequence=0,destroyed=false;
  onDestroy(()=>{destroyed=true;sequence++;});
  async function read() {
    if(busy||!connected||!device||destroyed)return;
    const request=++sequence,target=device;
    const current=()=>!destroyed&&sequence===request&&device===target&&connected;
    busy=true;snapshot=null;failure=null;diagnostic='';
    try {
      const response=await rpc<unknown>('get_firmware',{dev:target});
      if(!current())return;
      const parsed=firmwareInfo(response);
      if(!parsed){failure='invalid';return;}
      snapshot=parsed;
    } catch(error) {if(current()){failure='read';diagnostic=message(error);}}
    finally {if(current())busy=false;}
  }
</script>

<section aria-label={t('Сведения о прошивке')} aria-busy={busy}>
  <h2>{t('Сведения о прошивке')}</h2>
  <p>{t('Чтение сведений не меняет настройки устройства.')}</p>
  <p>{t('Устройство:')} <code>{device||'—'}</code></p>
  <button disabled={!connected||!device} aria-disabled={busy} onclick={read}>{t('Прочитать сведения о прошивке')}</button>
  {#if !connected||!device}<p role="status">{t('Подключитесь к daemon и выберите доступное устройство.')}</p>
  {:else if busy}<p role="status">{t('Читаем сведения о прошивке…')}</p>
  {:else if failure}<p role="alert">{failure==='invalid'?t('Daemon вернул неподдерживаемые сведения о прошивке.'):t('Не удалось прочитать сведения о прошивке. Повторите чтение явно.')}</p>
  {:else if !snapshot}<p role="status">{t('Сведения о прошивке ещё не прочитаны.')}</p>
  {:else}<p role="status">{t('Сведения о прошивке прочитаны.')}</p>{/if}
  {#if snapshot}
    <dl><dt>{t('Версия прошивки')}</dt><dd>{snapshot.version}</dd>
      <dt>{t('Дата сборки устройства')}</dt><dd>{snapshot.build_datetime??t('Недоступна')}</dd></dl>
    {#if !snapshot.build_datetime}<p>{t('Устройство не сообщило дату сборки.')}</p>{/if}
    <details><summary>{t('Идентификация устройства')}</summary><pre>{snapshot.raw_token}</pre></details>
  {/if}
  {#if diagnostic}<details><summary>{t('Техническая диагностика')}</summary><pre>{diagnostic}</pre></details>{/if}
</section>

<style>
  section{max-width:760px;display:grid;gap:14px;margin-block:20px;padding:20px;border:1px solid var(--border);border-radius:12px}
  h2{font-size:19px}p{line-height:1.6;color:var(--text-muted)}button{justify-self:start}button[aria-disabled=true]{opacity:.6}
  dl{display:grid;grid-template-columns:minmax(100px,1fr) minmax(0,2fr);gap:10px}dd{margin:0;overflow-wrap:anywhere}
  code,pre{user-select:text;overflow-wrap:anywhere}pre{white-space:pre-wrap}summary{cursor:pointer}
  [role=alert]{color:var(--danger)}[role=status]{color:var(--success)}
  @media(max-width:560px){dl{grid-template-columns:1fr}}
</style>
