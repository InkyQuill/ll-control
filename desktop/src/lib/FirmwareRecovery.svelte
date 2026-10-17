<script lang="ts">
 import {onDestroy} from 'svelte';
 import {t} from './i18n.svelte';
 import {rpc,message} from './api';
 import {firmwareRecovery,type FirmwareRecovery,type RecoveryState,type SavedStage} from './firmware-recovery';
 let snapshot=$state<FirmwareRecovery|null>(null),busy=$state(false),failure=$state<'read'|'invalid'|null>(null),diagnostic=$state('');
 let sequence=0,destroyed=false;
 onDestroy(()=>{destroyed=true;sequence++;});
 async function read(){
  if(busy||destroyed)return;
  const serial=++sequence;const current=()=>!destroyed&&serial===sequence;
  busy=true;snapshot=null;failure=null;diagnostic='';
  try{const response=await rpc<unknown>('get_firmware_recovery');if(!current())return;
   const parsed=firmwareRecovery(response);if(!parsed){failure='invalid';diagnostic=JSON.stringify(response)?.slice(0,16384)??'';return;}snapshot=parsed;
  }catch(error){if(current()){failure='read';diagnostic=message(error);}}
  finally{if(current())busy=false;}
 }
 function stateLabel(state:RecoveryState){switch(state){
  case 'no_saved_operation':return t('Сохранённый журнал и блокировки прошивки не найдены.');
  case 'journal_only':return t('Есть незавершённая запись в журнале прошивки. Отсутствие отметок не доказывает отсутствие записи на устройство.');
  case 'guarded':return t('Сохранённая операция обновления прошивки заблокировала восстановление настроек. Нужны проверка устройства и разрешение исхода операции.');
  case 'guard_only':return t('Есть блокировка настроек без журнала прошивки. Исход операции неизвестен.');
  case 'inconsistent':return t('Журнал прошивки и блокировки настроек не согласованы. Исход операции неизвестен.');
  case 'unreadable':return t('Сохранённое состояние прошивки не удалось прочитать. Исход операции неизвестен.');
 }}
 function stageLabel(stage:SavedStage){switch(stage){
  case 'armed':return t('Сохранено: операция подготовлена');
  case 'effects_possible':return t('Сохранено: команды могли изменить устройство');
  case 'not_started':return t('Сохранено: передача не начата');
  case 'unknown':return t('Сохранено: исход неизвестен');
  case 'awaiting_reconnect':return t('Сохранено: ожидание переподключения');
 }}
 const target=(value:number)=>value.toString(16).padStart(4,'0');
</script>
<section aria-label={t('Сохранённое состояние прошивки')} aria-busy={busy}>
 <h2>{t('Сохранённое состояние прошивки')}</h2>
 <p>{t('Это снимок сохранённых файлов, а не текущее состояние операции или подтверждение успешного обновления. Для нового снимка нажмите чтение ещё раз.')}</p>
 <p>{t('Чтение доступно без подключённого устройства и при блокировке обычных команд daemon.')}</p>
 <button aria-disabled={busy} onclick={read}>{t('Прочитать сохранённое состояние прошивки')}</button>
 {#if busy}<p role="status">{t('Читаем сохранённое состояние прошивки…')}</p>
 {:else if failure}<p role="alert">{failure==='invalid'?t('Daemon вернул неподдерживаемое сохранённое состояние прошивки.'):t('Не удалось прочитать сохранённое состояние прошивки. Повторите чтение явно.')}</p>
 {:else if snapshot}<p role={snapshot.state==='no_saved_operation'?'status':'alert'}>{stateLabel(snapshot.state)}</p>
 {:else}<p role="status">{t('Сохранённое состояние прошивки ещё не прочитано.')}</p>{/if}
 {#if snapshot}
  {#if snapshot.state!=='no_saved_operation'}<p>{t('Сохраните журнал и блокировки для диагностики. Это чтение не снимает блокировку, не повторяет передачу и не восстанавливает настройки устройства.')}</p>{/if}
  {#if snapshot.operation}
   {@const operation=snapshot.operation}
   <dl><dt>{t('ID сохранённой операции обновления прошивки')}</dt><dd><code>{operation.operation_id}</code></dd>
    <dt>{t('Сохранённый этап')}</dt><dd>{stageLabel(operation.stage)}</dd>
    <dt>{t('Сохранённая цель VID:PID')}</dt><dd><code>{target(operation.vid)}:{target(operation.pid)}</code></dd>
    <dt>{t('Сохранённый серийный номер')}</dt><dd>{operation.serial??t('Серийный номер не сохранён')}</dd>
    <dt>{t('Предыдущий HID-путь')}</dt><dd><code>{operation.previous_hid_path}</code></dd>
    <dt>{t('Ключ сохранённых настроек')}</dt><dd><code>{operation.settings_key}</code></dd>
    <dt>{t('SHA-256 сохранённого образа')}</dt><dd><code>{operation.image_sha256}</code></dd></dl>
   {#if operation.progress}
    {@const progress=operation.progress}
    <h3>{t('Сохранённые отметки передачи')}</h3>
    <p>{t('Отметки могут отставать от фактических команд после сбоя. Они не доказывают завершение операции или отсутствие других записей.')}</p>
    <dl><dt>{t('Пакетов в сохранённом плане')}</dt><dd>{progress.total_chunks}</dd>
     <dt>{t('Сохранённых отметок отправки')}</dt><dd>{progress.chunk_writes}</dd>
     <dt>{t('Сохранённых отметок ответа')}</dt><dd>{progress.chunk_echoes}</dd>
     <dt>{t('Отметка команды помпы')}</dt><dd>{progress.pump_hold_written?t('Отметка сохранена'):t('Отметка не сохранена')}</dd>
     <dt>{t('Отметка команды вентиляторов')}</dt><dd>{progress.fan_hold_written?t('Отметка сохранена'):t('Отметка не сохранена')}</dd>
     <dt>{t('Отметка команды активации')}</dt><dd>{progress.activation_written?t('Отметка сохранена'):t('Отметка не сохранена')}</dd></dl>
   {/if}
  {/if}
  {#if snapshot.guards.length}
   <h3>{t('Сохранённые блокировки настроек')}</h3>
   <ul>{#each snapshot.guards as guard}<li><dl><dt>{t('Ключ сохранённых настроек')}</dt><dd><code>{guard.settings_key}</code></dd><dt>{t('ID сохранённой операции обновления прошивки')}</dt><dd><code>{guard.operation_id}</code></dd><dt>{t('SHA-256 сохранённого образа')}</dt><dd><code>{guard.image_sha256}</code></dd></dl></li>{/each}</ul>
  {/if}
  <details><summary>{t('Сохранённая диагностика прошивки')}</summary><textarea readonly rows="12" aria-label={t('Сохранённая диагностика прошивки')} value={JSON.stringify(snapshot,null,2)}></textarea></details>
 {/if}
 {#if diagnostic}<details><summary>{t('Техническая диагностика')}</summary><pre>{diagnostic}</pre></details>{/if}
</section>
<style>
 section{max-width:760px;display:grid;gap:14px;margin-block:20px;padding:20px;border:1px solid var(--border);border-radius:12px}h2{font-size:19px}h3{font-size:16px}p{line-height:1.6;color:var(--text-muted)}button{justify-self:start}button[aria-disabled=true]{opacity:.6}dl{display:grid;grid-template-columns:minmax(100px,1fr) minmax(0,2fr);gap:10px}dd{margin:0;overflow-wrap:anywhere}ul{padding-left:20px}pre{white-space:pre-wrap;overflow-wrap:anywhere}textarea{box-sizing:border-box;width:100%;font-family:monospace;background:var(--canvas);color:inherit;resize:vertical}button:focus-visible,textarea:focus-visible,summary:focus-visible{outline:3px solid var(--accent);outline-offset:3px}
 @media(max-width:540px){dl{grid-template-columns:1fr}}
</style>
