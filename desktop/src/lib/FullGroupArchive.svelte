<script lang="ts">
  import {t} from './i18n.svelte';
  import WindowsScreenPreview from './WindowsScreenPreview.svelte';
  import type {DisplayGeometry} from './device-capabilities';
  import WindowsScreenBindings from './WindowsScreenBindings.svelte';
  import type {ScreenOptions} from './windows-screen';
  import type {FontChoice} from './font-upload';
  import WindowsLightingReview from './WindowsLightingReview.svelte';
  import {windowsRoles,windowsBindingInvalid,windowsSourceUnverified,windowsUnit,windowsUnitLabel,windowsRoleLabel,windowsOptions,windowsOptionsKey,emptyWindowsIds,type WindowsIds} from './windows-control';
  import {ArchiveUiError,archiveError,archiveMessage,archiveDiagnostic,type ArchiveMessage} from './archive-messages';
  import {presentation} from './presentation.svelte';
  import {onDestroy} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  import {coolingModeLabel} from './cooling-modes';
  import {type Sensor} from './api';
  import {policySummary,groupNames,groupPhases,groupIdle,groupPendingKey,readGroupPending,type FullGroup,type GroupTarget,type GroupMode,type GroupStatus} from './group-archive';
  let {device,connected,sensors=[],fonts=[],geometry=null,disabled=false,onbusy,onchange}:{device:string;connected:boolean;sensors?:Sensor[];fonts?:FontChoice[];geometry?:DisplayGeometry|null;disabled?:boolean;onbusy:(value:boolean)=>void;onchange:(dev:string)=>void}=$props();
  const initial=readGroupPending();
  let pending=$state<GroupTarget|null>(initial),job=$state<GroupStatus|null>(null);
  let group=$state<FullGroup>(initial?.group??'screen'),recoveryId=$state(initial?.id??'');
  let recoveryDev=$state(initial?.dev??''),recoveryMode=$state<GroupMode>(initial?.mode??'import');
  let resetSensor=$state('');
  let windowsRecord=$state(''),windowsIds=$state<WindowsIds>(emptyWindowsIds());
  let windowsFormFor='';
  const windowsDraft=$derived(windowsOptions(windowsRecord,windowsIds,sensors,job?.windows_source?.controllers.map(c=>c.record_index)??[]));
  const windowsCurrent=$derived(!!windowsDraft&&windowsOptionsKey(windowsDraft)===windowsOptionsKey(job?.windows_options));
  const temperatureSensors=$derived(sensors.filter(s=>s.unit==='celsius'));
  let open=$state(false),requesting=$state(false),screenPreviewBusy=$state(false);
  let error=$state<string|ArchiveMessage>('');
  let mutation=0,handled='';
  const active=$derived(requesting||!!job&&!groupIdle(job.phase));
  const windowsEditable=$derived(!!job&&job.mode==='windows'&&job.dev===device&&connected&&!disabled&&!active&&['bindings','prepared','failed'].includes(job.phase));
  const blocked=$derived(active||disabled||!connected||!device||!!pending);
  const target=$derived<GroupTarget>({id:recoveryId,dev:recoveryDev||device,group,mode:recoveryMode});
  const recoveryValid=$derived(/^[a-f0-9]{32}$/.test(target.id)&&!!target.dev&&(!['windows','reset'].includes(target.mode)||target.group==='fan_and_lighting')&&(target.mode!=='windows_screen'||target.group==='screen'));
  const applicable=$derived(!!job&&job.dev===device&&job.group===group&&connected&&!disabled&&!active);
  const cancellable=$derived(!!job&&(!['reset','windows'].includes(job.mode)||['bindings','prepared','inspecting'].includes(job.phase))&&!['applying','recovering','cancelling','complete','cancelled','failed'].includes(job.phase)&&!requesting);
  $effect(()=>{onbusy(active||screenPreviewBusy);});
  onDestroy(()=>onbusy(false));
  function remember(value:GroupTarget) {
    const current=readGroupPending();
    if(current&&(current.id!==value.id||current.dev!==value.dev||current.group!==value.group||current.mode!==value.mode))throw new ArchiveUiError('pending_group_operation');
    localStorage.setItem(groupPendingKey,JSON.stringify(value));pending=value;
    recoveryId=value.id;recoveryDev=value.dev;group=value.group;recoveryMode=value.mode;
  }
  function accept(status:GroupStatus|null) {
    if(!status)return;
    if(pending&&(status.id!==pending.id||status.dev!==pending.dev||status.group!==pending.group||status.mode!==pending.mode)) {error='В приложении есть другая операция группы. Сначала завершите её восстановление.';return;}
    if(!pending)remember({id:status.id,dev:status.dev,group:status.group,mode:status.mode});
    if(status.mode==='windows'&&windowsFormFor!==status.id) {
      windowsFormFor=status.id;windowsRecord=status.windows_options?String(status.windows_options.record):'';windowsIds=emptyWindowsIds();
      if(status.windows_options)for(const role of windowsRoles)windowsIds[role]=status.windows_options.bindings[role].sensor_id;
    }
    job=status;
    if(status.readback) {
      const key=JSON.stringify([status.id,status.readback.receipt,status.readback.runtime]);
      if(key!==handled){handled=key;onchange(status.dev);}
    }
  }
  $effect(()=>{
    if(!connected||!(open||active))return;
    let stopped=false;let timer:ReturnType<typeof setTimeout>;
    async function poll(){const epoch=mutation;try {const result=await invoke<GroupStatus|null>('group_archive_status');if(!stopped&&epoch===mutation)accept(result);}catch(e){if(!stopped&&epoch===mutation)error=archiveError(e);}if(!stopped)timer=setTimeout(poll,500);}
    timer=setTimeout(poll,0);return()=>{stopped=true;clearTimeout(timer);};
  });
  async function start(mode:GroupMode) {
    if(blocked||mode==='reset'&&(group!=='fan_and_lighting'||!resetSensor)||mode==='windows'&&group!=='fan_and_lighting'||mode==='windows_screen'&&group!=='screen')return;
    const request:GroupTarget={id:crypto.randomUUID().replaceAll('-',''),dev:device,group,mode};
    mutation++;requesting=true;error='';
    try {
      const current=await invoke<GroupStatus|null>('group_archive_status');
      if(current&&(!groupIdle(current.phase)||current.phase==='prepared'||current.phase==='uncertain')) {accept(current);return;}
      remember(request);accept(await invoke<GroupStatus>('group_archive_start',{language:presentation.language,request:mode==='reset'?{...request,sensor_id:resetSensor}:request}));
    }catch(e){error=archiveError(e);}finally{requesting=false;mutation++;}
  }
  async function prepareWindows() {
    if(!windowsEditable||!windowsDraft||!job)return;
    const request:GroupTarget={id:job.id,dev:job.dev,group:job.group,mode:'windows'};
    mutation++;requesting=true;error='';
    try {accept(await invoke<GroupStatus>('group_windows_prepare',{request,options:windowsDraft}));}
    catch(e){const diagnostic=archiveError(e);error=diagnostic.message_code?diagnostic:{...diagnostic,message_code:'windows_preparation_failed'};}finally{requesting=false;mutation++;}
  }
  async function prepareScreen(options:ScreenOptions|{record:number;inspect_only:true}) {
    if(!job||job.mode!=='windows_screen'||job.phase!=='bindings'||!applicable)return;
    const request:GroupTarget={id:job.id,dev:job.dev,group:job.group,mode:job.mode};
    mutation++;requesting=true;error='';
    try{accept(await invoke<GroupStatus>('group_windows_prepare',{request,options}));}
    catch(e){error=archiveError(e);}finally{requesting=false;mutation++;}
  }
  async function act(action:'finish'|'recover'|'cancel'|'reapply'|'dismiss') {
    if(requesting||!connected&&action!=='dismiss'&&!(action==='cancel'&&job?.mode==='windows')||!recoveryValid)return;
    if(['finish','reapply'].includes(action)&&(!applicable||screenPreviewBusy))return;
    if(action==='finish'&&job?.mode==='windows'&&job.phase==='prepared'&&!windowsCurrent)return;
    const request:GroupTarget={...target};
    mutation++;requesting=true;error='';
    try {
      remember(request);
      const status=await invoke<GroupStatus|null>('group_archive_action',{request,action,expected:action==='reapply'?job?.readback?.runtime.revision??null:null});
      if(action==='dismiss') {
        if(readGroupPending()?.id===request.id)localStorage.removeItem(groupPendingKey);
        pending=readGroupPending();job=null;recoveryId='';recoveryDev='';handled='';
      }else accept(status);
    }catch(e){error=archiveError(e);}finally{requesting=false;mutation++;}
  }
</script>
<details ontoggle={e=>open=e.currentTarget.open}>
  <summary>{t("Резервная копия группы настроек")}</summary>
  <p>{t("Архив L-Connect Linux (.lcg) сохраняет всю выбранную группу и её слоты. «Экран» включает изображения, видео и шрифты. Windows ZIP 2.1.28 можно импортировать отдельно для экрана и FanAndLighting.")}</p>
  <label>{t("Группа настроек")}<select aria-label="{t("Группа настроек")}" bind:value={group} disabled={active||!!pending}><option value="screen">{t("Экран")}</option><option value="fan_and_lighting">{t("Вентиляторы и подсветка")}</option></select></label>
  <p>{t("Устройство:")} <code>{pending?.dev||device||t('не выбрано')}</code> · {t(groupNames[group])}</p>
  {#if group==='fan_and_lighting'}
    <label>{t("Температурный датчик CPU для сброса")}<select aria-label="{t("Температурный датчик CPU для сброса")}" bind:value={resetSensor} disabled={blocked}>
      <option value="">{t("Выберите датчик температуры CPU")}</option>
      {#if resetSensor&&!temperatureSensors.some(s=>s.id===resetSensor)}<option value={resetSensor}>{resetSensor} {t("· недоступен")}</option>{/if}
      {#each temperatureSensors as sensor (sensor.id)}<option value={sensor.id}>{sensor.label} · {sensor.id}</option>{/each}
    </select></label>
    <p>{t("Полный сброс вернёт стандартные 8 режимов вентиляторов и 3 режима помпы, их имена и выбор Fan Standard / Pump Quiet RPM, Rainbow, topology 360 мм/24 LED и восемь named fan/pump-слотов. Screen, файлы и общая thermal policy сохранятся. При недоступном датчике действует fallback 100%.")}</p>
    <button disabled={blocked||!resetSensor} onclick={()=>start('reset')}>{t("Подготовить полный сброс охлаждения и подсветки…")}</button>
  {/if}
  <div class="buttons">{#if group==='fan_and_lighting'}<button disabled={blocked} onclick={()=>start('windows')}>{t('Выбрать Windows ZIP…')}</button>{:else}<button disabled={blocked} onclick={()=>start('windows_screen')}>{t('Выбрать Windows ZIP экрана…')}</button>{/if}<button disabled={blocked} onclick={()=>start('export')}>{t("Экспортировать группу…")}</button><button disabled={blocked} onclick={()=>start('import')}>{t("Выбрать архив группы…")}</button></div>
  <p>{t("Существующие файлы при экспорте не заменяются. Выбор и проверка архива не применяют настройки.")}</p>
  {#if job}
    <p role="status">{job.mode==='reset'&&job.phase==='prepared'?"" + t("Сброс подготовлен, ожидает подтверждения") + "":t(groupPhases[job.phase])}{job.file_name?` · ${job.file_name}`:''}</p>
    {#if active}<progress aria-label={job.mode==='reset'?t('Операция сброса'):t('Передача группы')} max={job.total||1} value={job.total?job.transferred:undefined}></progress>{#if !['reset','windows'].includes(job.mode)}<p>{job.transferred.toLocaleString(presentation.language)} / {job.total?job.total.toLocaleString(presentation.language):'…'} {t("байт")}</p>{/if}{/if}
    {#if job.mode==='windows'&&job.windows_source}
      <section class="windows-bindings" aria-label={t('Привязки Windows FanAndLighting')}>
        <h3>{t('Исходный контроллер и датчики')}</h3>
        <p>{t('Применение выполняется к устройству, указанному выше. Выберите исходный контроллер в архиве и отдельный ID датчика для каждой роли. Недоступные ID сохраняются без автоматической замены.')}</p>
        <label>{t('Исходный контроллер Windows')}<select aria-label={t('Исходный контроллер Windows')} bind:value={windowsRecord} disabled={!windowsEditable}><option value="">{t('Выберите record контроллера')}</option>{#each job.windows_source.controllers as source (source.record_index)}<option value={String(source.record_index)}>{t('Record {0} · {1}',source.record_index,source.profile_sha256.slice(0,12))}</option>{/each}</select></label>
        <div class="windows-sources">{#each windowsRoles as role (role)}
          <div><label>{windowsRoleLabel(role)} ({windowsUnitLabel(role)})<input list={`windows-${job.id}-${role}`} bind:value={windowsIds[role]} aria-invalid={windowsBindingInvalid(role,windowsIds,sensors)} aria-describedby={windowsBindingInvalid(role,windowsIds,sensors)||windowsSourceUnverified(role,windowsIds,sensors)?`windows-${job.id}-${role}-hint`:undefined} disabled={!windowsEditable} maxlength="1024" spellcheck="false" placeholder={t('Выберите или введите ID датчика')}/></label>
          <datalist id={`windows-${job.id}-${role}`}>{#each sensors.filter(s=>s.unit===windowsUnit(role)) as sensor (sensor.id)}<option value={sensor.id}>{sensor.label}</option>{/each}</datalist>
          {#if windowsBindingInvalid(role,windowsIds,sensors)}<p id={`windows-${job.id}-${role}-hint`} role="alert">{t('ID повторяется, имеет неподходящие единицы или недопустимый формат.')}</p>{:else if windowsSourceUnverified(role,windowsIds,sensors)}<p id={`windows-${job.id}-${role}-hint`} class="source-note">{t('Источник недоступен или не подтверждён. ID сохранён без подмены.')}</p>{/if}</div>
        {/each}</div>
        <p>{t('Укажите шесть разных ID с нужными единицами. Без показаний температуры охлаждение использует 100%; сенсорная подсветка возвращается к обычным параметрам эффекта.')}</p>
        <button disabled={!windowsEditable||!windowsDraft} onclick={prepareWindows}>{t('Проверить настройки Windows')}</button>
        {#if job.phase==='prepared'&&!windowsCurrent}<p role="status">{t('Привязки изменились. Повторите подготовку перед применением.')}</p>{/if}
      </section>
    {/if}
    {#if job.mode==='windows_screen'&&job.windows_source}
      {#key job.id}<WindowsScreenBindings {job} {sensors} {fonts} editable={applicable} oninspect={record=>prepareScreen({record,inspect_only:true})} onprepare={prepareScreen}/>{/key}
    {/if}
    {#if job.summary}
      <div aria-label="{t("Содержимое группы")}">
        <p>{t("Слотов:")} {job.summary.slots.length}</p>
        {#each job.summary.slots as slot (`${slot.group}:${slot.slot}`)}<p>{slot.group} · {slot.slot+1}: {slot.name}</p>{/each}
        {#if job.summary.theme_name}<p>{t("Тема:")} {job.summary.theme_name}</p>{/if}
        {#if job.summary.media_key}<p>{t("Источник: видео")}</p>{/if}
        {#if job.summary.lcd}<p>LCD: {job.summary.lcd.brightness_percent}% · {job.summary.lcd.rotation_degrees}° · {job.summary.lcd.mode}</p>{/if}
        {#if job.summary.fan_manual_percent!==null}<p>{t("Вентиляторы:")} {job.summary.fan_manual_percent}%</p>{/if}
        {#if job.summary.pump_manual}<p>{t("Помпа:")} {job.summary.pump_manual.percent}% · {job.summary.pump_manual.source}</p>{/if}
        {#if job.summary.cooling_modes}
          <details class="bank-summary"><summary>{t('Сохранённые режимы: вентиляторы {0}, помпа {1}',job.summary.cooling_modes.fan.length,job.summary.cooling_modes.pump.length)}</summary>
            {#each job.summary.cooling_modes.fan as mode (mode.id)}<p>{t('Вентиляторы')}: {coolingModeLabel(mode)} ({mode.id}) · {policySummary(mode.policy,presentation.language)}</p>{/each}
            {#each job.summary.cooling_modes.pump as mode (mode.id)}<p>{t('Помпа')}: {coolingModeLabel(mode)} ({mode.id}) · {policySummary(mode.policy,presentation.language)}</p>{/each}
          </details>
        {/if}
        {#if job.summary.cooling?.fan_mode||job.summary.cooling?.pump_mode}<p>{t('ID выбранных режимов: вентиляторы {0}, помпа {1}',job.summary.cooling?.fan_mode??'—',job.summary.cooling?.pump_mode??'—')}</p>{/if}
        {#if job.summary.cooling}<p>{t("Политика вентиляторов:")} {policySummary(job.summary.cooling.fan,presentation.language)}</p><p>{t("Политика помпы:")} {policySummary(job.summary.cooling.pump,presentation.language)}</p>{/if}
        {#if job.summary.dynamic_rgb}<p>{t("Динамическая подсветка:")} {job.summary.dynamic_rgb.sensor_id} · {job.summary.dynamic_rgb.low_value}–{job.summary.dynamic_rgb.high_value} {job.summary.dynamic_rgb.unit}</p>{/if}
        {#if job.summary.fan_led_mode}<p>{t("Подсветка вентиляторов:")} {job.summary.fan_led_mode}</p>{/if}
        {#if job.summary.pump_led_mode}<p>{t("Подсветка помпы:")} {job.summary.pump_led_mode}</p>{/if}
      </div>
    {/if}
    {#if job.windows_lighting}<WindowsLightingReview review={job.windows_lighting}/>{/if}
    {#if job.mode==='windows'&&job.phase==='prepared'}<p>{t('Импорт заменит режимы охлаждения и подсветки. Сохранённые Linux-профили, настройки экрана, файлы и температурные оповещения останутся. Применение начнётся после подтверждения.')}</p><button disabled={!applicable||!windowsCurrent} onclick={()=>act('finish')}>{t('Импортировать и применить')}</button>{/if}
    {#if job.mode==='windows_screen'&&job.phase==='prepared'}<WindowsScreenPreview target={job} {geometry} enabled={applicable} onbusy={value=>screenPreviewBusy=value}/>{/if}
    {#if job.mode==='windows_screen'&&job.phase==='prepared'}<p>{t('Импорт заменит текущую сцену LCD, редактор, яркость, поворот и галереи. Сохранённые Linux-профили и настройки охлаждения останутся. Изображение на устройстве проверяется отдельно.')}</p><button disabled={!applicable||screenPreviewBusy} onclick={()=>act('finish')}>{t('Импортировать экран и применить')}</button>{/if}
    {#if job.phase==='prepared'&&!['windows','windows_screen'].includes(job.mode)}<p>{job.mode==='reset'?"" + t("Сброс") + "":"" + t("Восстановление") + ""} {t("заменит сохранённые настройки всей группы и её слоты и сразу применит их к указанному устройству. Другая группа сохранится. Открытые черновики редакторов сохранятся.")}</p><button disabled={!applicable} onclick={()=>act('finish')}>{job.mode==='reset'?"" + t("Подтвердить полный сброс и применить") + "":"" + t("Восстановить и применить группу") + ""}</button>{/if}
    {#if job.mode==='reset'&&job.phase==='uncertain'&&job.snapshot&&job.sensor_id}<p>{t("Ответ на сброс не подтверждён. Повтор использует тот же ID, датчик и исходную версию настроек; новый сброс автоматически не запускается.")}</p><button disabled={!applicable} onclick={()=>act('finish')}>{t("Повторить запрос сброса с тем же ID")}</button>{/if}
    {#if job.mode==='windows'&&job.phase==='uncertain'&&job.snapshot&&job.windows_options}<p>{t('Ответ на импорт не подтверждён. Повтор использует подготовленные настройки и тот же ID операции; правки формы в этот запрос не попадут.')}</p><button disabled={!applicable} onclick={()=>act('finish')}>{t('Повторить импорт с тем же ID')}</button>{/if}
    {#if job.phase==='applying'}<p>{t("Применение уже началось. Дождитесь результата; после потери связи прочитайте его по ID операции.")}</p>{/if}
    {#if job.phase==='complete'&&job.mode==='export'}<p role="status">{t("Архив группы сохранён в файл.")}</p>{/if}
    {#if job.readback}
      <p role="status">{t("Сохранение группы подтверждено.")} {job.readback.selected_content_current?"" + t("Сохранённое содержимое группы соответствует этой операции.") + "":"" + t("После операции настройки уже изменились.") + ""}</p>
      {#if job.readback.runtime.recovery_required}<p role="alert">{t("Runtime требует восстановления. Перезапустите daemon и прочитайте результат заново.")}</p>{/if}
      {#if job.readback.runtime.last_error}<p role="alert">{t("Настройки сохранены, при применении возникла ошибка:")} {job.readback.runtime.last_error}</p>{/if}
      <button disabled={!applicable||screenPreviewBusy||!job.readback.selected_content_current||!job.readback.runtime.revision||!!job.readback.runtime.recovery_required} onclick={()=>act('reapply')}>{t("Повторить применение сохранённой группы")}</button>
    {/if}
    {#if job.message||job.message_code}<p role={['failed','uncertain'].includes(job.phase)?'alert':'status'}>{archiveMessage(job,presentation.language)}</p>{#if archiveDiagnostic(job,presentation.language)}<details><summary>{t('Техническая диагностика')}</summary><pre>{archiveDiagnostic(job,presentation.language)}</pre></details>{/if}{/if}
    {#if cancellable}<button onclick={()=>act('cancel')}>{job.mode==='windows'?t('Отменить Windows подготовку'):job.mode==='reset'?"" + t("Отменить подготовку сброса") + "":"" + t("Отменить передачу группы") + ""}</button>{/if}
  {/if}
  <label>{t("ID операции группы")}<input bind:value={recoveryId} maxlength="32" spellcheck="false" disabled={active||!!pending}/></label>
  <label>{t("Устройство для чтения результата")}<input bind:value={recoveryDev} placeholder={device} disabled={active||!!pending}/></label>
  <label>{t("Тип операции")}<select aria-label="{t("Тип операции")}" bind:value={recoveryMode} disabled={active||!!pending}><option value="import">{t("Импорт")}</option><option value="export">{t("Экспорт")}</option><option value="reset">{t("Полный сброс Fan + Lighting")}</option><option value="windows">{t("Импорт Windows FanAndLighting")}</option><option value="windows_screen">{t("Импорт Windows Screen")}</option></select></label>
  <div class="buttons">
    <button disabled={active||!connected||!recoveryValid} onclick={()=>act('recover')}>{t("Прочитать результат группы")}</button>
    {#if pending||job}<button disabled={active||job?.phase==='prepared'} onclick={()=>act('dismiss')}>{t("Закрыть сведения о группе")}</button>{/if}
    {#if pending&&!job&&!['reset','windows'].includes(pending.mode)}<button disabled={active||!connected} onclick={()=>act('cancel')}>{t("Отменить сохранённую передачу")}</button>{/if}
  </div>
  {#if pending}<p>{t("ID сохранён для восстановления:")} <code>{pending.id}</code>{t(". Закрытие сведений не отменяет уже принятую операцию.")}</p>{/if}
  {#if error}<p role="alert">{archiveMessage(error,presentation.language)}</p>{#if archiveDiagnostic(error,presentation.language)}<details><summary>{t('Техническая диагностика')}</summary><pre>{archiveDiagnostic(error,presentation.language)}</pre></details>{/if}{/if}
</details>
<style>
  .windows-sources{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:0 18px}.source-note{color:var(--text-muted);font-size:12px}.windows-bindings{border-top:1px solid var(--border);padding-top:20px;margin-top:20px}.windows-sources input{min-width:0;width:100%}@media(max-width:800px){.windows-sources{grid-template-columns:1fr}}
  pre{white-space:pre-wrap;overflow-wrap:anywhere;user-select:text}
  details{border:1px solid var(--border);border-radius:12px;padding:18px;margin-block:16px}summary{cursor:pointer;font-weight:600}p{line-height:1.5}label{display:grid;gap:6px;margin:12px 0}.buttons{display:flex;flex-wrap:wrap;gap:8px}progress{width:100%}[role=alert]{color:var(--danger)}code{user-select:all;overflow-wrap:anywhere}
</style>
