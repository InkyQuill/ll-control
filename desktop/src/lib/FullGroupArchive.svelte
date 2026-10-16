<script lang="ts">
  import {t} from './i18n.svelte';
  import {ArchiveUiError,archiveError,archiveMessage,archiveDiagnostic,type ArchiveMessage} from './archive-messages';
  import {presentation} from './presentation.svelte';
  import {onDestroy} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  import {type Sensor} from './api';
  import {policySummary,groupNames,groupPhases,groupIdle,groupPendingKey,readGroupPending,type FullGroup,type GroupTarget,type GroupMode,type GroupStatus} from './group-archive';
  let {device,connected,sensors=[],disabled=false,onbusy,onchange}:{device:string;connected:boolean;sensors?:Sensor[];disabled?:boolean;onbusy:(value:boolean)=>void;onchange:(dev:string)=>void}=$props();
  const initial=readGroupPending();
  let pending=$state<GroupTarget|null>(initial),job=$state<GroupStatus|null>(null);
  let group=$state<FullGroup>(initial?.group??'screen'),recoveryId=$state(initial?.id??'');
  let recoveryDev=$state(initial?.dev??''),recoveryMode=$state<GroupMode>(initial?.mode??'import');
  let resetSensor=$state('');
  const temperatureSensors=$derived(sensors.filter(s=>s.unit==='celsius'));
  let open=$state(false),requesting=$state(false);
  let error=$state<string|ArchiveMessage>('');
  let mutation=0,handled='';
  const active=$derived(requesting||!!job&&!groupIdle(job.phase));
  const blocked=$derived(active||disabled||!connected||!device||!!pending);
  const target=$derived<GroupTarget>({id:recoveryId,dev:recoveryDev||device,group,mode:recoveryMode});
  const recoveryValid=$derived(/^[a-f0-9]{32}$/.test(target.id)&&!!target.dev);
  const applicable=$derived(!!job&&job.dev===device&&job.group===group&&connected&&!disabled&&!active);
  const cancellable=$derived(!!job&&(job.mode!=='reset'||['prepared','inspecting'].includes(job.phase))&&!['applying','recovering','cancelling','complete','cancelled','failed'].includes(job.phase)&&!requesting);
  $effect(()=>{onbusy(active);});
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
    if(blocked||mode==='reset'&&(group!=='fan_and_lighting'||!resetSensor))return;
    const request:GroupTarget={id:crypto.randomUUID().replaceAll('-',''),dev:device,group,mode};
    mutation++;requesting=true;error='';
    try {
      const current=await invoke<GroupStatus|null>('group_archive_status');
      if(current&&(!groupIdle(current.phase)||current.phase==='prepared'||current.phase==='uncertain')) {accept(current);return;}
      remember(request);accept(await invoke<GroupStatus>('group_archive_start',{language:presentation.language,request:mode==='reset'?{...request,sensor_id:resetSensor}:request}));
    }catch(e){error=archiveError(e);}finally{requesting=false;mutation++;}
  }
  async function act(action:'finish'|'recover'|'cancel'|'reapply'|'dismiss') {
    if(requesting||!connected&&action!=='dismiss'||!recoveryValid)return;
    if(['finish','reapply'].includes(action)&&!applicable)return;
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
  <p>{t("Архив L-Connect Linux (.lcg) сохраняет всю выбранную группу и её слоты. «Экран» включает изображения, видео и шрифты. Формат Windows ZIP пока не поддерживается.")}</p>
  <label>{t("Группа настроек")}<select aria-label="{t("Группа настроек")}" bind:value={group} disabled={active||!!pending}><option value="screen">{t("Экран")}</option><option value="fan_and_lighting">{t("Вентиляторы и подсветка")}</option></select></label>
  <p>{t("Устройство:")} <code>{pending?.dev||device||t('не выбрано')}</code> · {t(groupNames[group])}</p>
  {#if group==='fan_and_lighting'}
    <label>{t("Температурный датчик CPU для сброса")}<select aria-label="{t("Температурный датчик CPU для сброса")}" bind:value={resetSensor} disabled={blocked}>
      <option value="">{t("Выберите датчик температуры CPU")}</option>
      {#if resetSensor&&!temperatureSensors.some(s=>s.id===resetSensor)}<option value={resetSensor}>{resetSensor} {t("· недоступен")}</option>{/if}
      {#each temperatureSensors as sensor (sensor.id)}<option value={sensor.id}>{sensor.label} · {sensor.id}</option>{/each}
    </select></label>
    <p>{t("Полный сброс вернёт Fan Standard, Pump Quiet RPM, Rainbow, topology 360 мм/24 LED и все восемь fan/pump-слотов. Screen, файлы и общая thermal policy сохранятся. При недоступном датчике действует fallback 100%.")}</p>
    <button disabled={blocked||!resetSensor} onclick={()=>start('reset')}>{t("Подготовить полный сброс охлаждения и подсветки…")}</button>
  {/if}
  <div class="buttons"><button disabled={blocked} onclick={()=>start('export')}>{t("Экспортировать группу…")}</button><button disabled={blocked} onclick={()=>start('import')}>{t("Выбрать архив группы…")}</button></div>
  <p>{t("Существующие файлы при экспорте не заменяются. Выбор и проверка архива не применяют настройки.")}</p>
  {#if job}
    <p role="status">{job.mode==='reset'&&job.phase==='prepared'?"" + t("Сброс подготовлен, ожидает подтверждения") + "":t(groupPhases[job.phase])}{job.file_name?` · ${job.file_name}`:''}</p>
    {#if active}<progress aria-label={job.mode==='reset'?t('Операция сброса'):t('Передача группы')} max={job.total||1} value={job.total?job.transferred:undefined}></progress>{#if job.mode!=='reset'}<p>{job.transferred.toLocaleString(presentation.language)} / {job.total?job.total.toLocaleString(presentation.language):'…'} {t("байт")}</p>{/if}{/if}
    {#if job.summary}
      <div aria-label="{t("Содержимое группы")}">
        <p>{t("Слотов:")} {job.summary.slots.length}</p>
        {#each job.summary.slots as slot (`${slot.group}:${slot.slot}`)}<p>{slot.group} · {slot.slot+1}: {slot.name}</p>{/each}
        {#if job.summary.theme_name}<p>{t("Тема:")} {job.summary.theme_name}</p>{/if}
        {#if job.summary.media_key}<p>{t("Источник: видео")}</p>{/if}
        {#if job.summary.lcd}<p>LCD: {job.summary.lcd.brightness_percent}% · {job.summary.lcd.rotation_degrees}° · {job.summary.lcd.mode}</p>{/if}
        {#if job.summary.fan_manual_percent!==null}<p>{t("Вентиляторы:")} {job.summary.fan_manual_percent}%</p>{/if}
        {#if job.summary.pump_manual}<p>{t("Помпа:")} {job.summary.pump_manual.percent}% · {job.summary.pump_manual.source}</p>{/if}
        {#if job.summary.cooling}<p>{t("Политика вентиляторов:")} {policySummary(job.summary.cooling.fan,presentation.language)}</p><p>{t("Политика помпы:")} {policySummary(job.summary.cooling.pump,presentation.language)}</p>{/if}
        {#if job.summary.dynamic_rgb}<p>{t("Динамическая подсветка:")} {job.summary.dynamic_rgb.sensor_id} · {job.summary.dynamic_rgb.low_value}–{job.summary.dynamic_rgb.high_value} {job.summary.dynamic_rgb.unit}</p>{/if}
        {#if job.summary.fan_led_mode}<p>{t("Подсветка вентиляторов:")} {job.summary.fan_led_mode}</p>{/if}
        {#if job.summary.pump_led_mode}<p>{t("Подсветка помпы:")} {job.summary.pump_led_mode}</p>{/if}
      </div>
    {/if}
    {#if job.phase==='prepared'}<p>{job.mode==='reset'?"" + t("Сброс") + "":"" + t("Восстановление") + ""} {t("заменит сохранённые настройки всей группы и её слоты и сразу применит их к указанному устройству. Другая группа сохранится. Открытые черновики редакторов сохранятся.")}</p><button disabled={!applicable} onclick={()=>act('finish')}>{job.mode==='reset'?"" + t("Подтвердить полный сброс и применить") + "":"" + t("Восстановить и применить группу") + ""}</button>{/if}
    {#if job.mode==='reset'&&job.phase==='uncertain'&&job.snapshot&&job.sensor_id}<p>{t("Ответ на сброс не подтверждён. Повтор использует тот же ID, датчик и исходную версию настроек; новый сброс автоматически не запускается.")}</p><button disabled={!applicable} onclick={()=>act('finish')}>{t("Повторить запрос сброса с тем же ID")}</button>{/if}
    {#if job.phase==='applying'}<p>{t("Применение уже началось. Дождитесь результата; после потери связи прочитайте его по ID операции.")}</p>{/if}
    {#if job.phase==='complete'&&job.mode==='export'}<p role="status">{t("Архив группы сохранён в файл.")}</p>{/if}
    {#if job.readback}
      <p role="status">{t("Сохранение группы подтверждено.")} {job.readback.selected_content_current?"" + t("Сохранённое содержимое группы соответствует этой операции.") + "":"" + t("После операции настройки уже изменились.") + ""}</p>
      {#if job.readback.runtime.recovery_required}<p role="alert">{t("Runtime требует восстановления. Перезапустите daemon и прочитайте результат заново.")}</p>{/if}
      {#if job.readback.runtime.last_error}<p role="alert">{t("Настройки сохранены, при применении возникла ошибка:")} {job.readback.runtime.last_error}</p>{/if}
      <button disabled={!applicable||!job.readback.selected_content_current||!job.readback.runtime.revision||!!job.readback.runtime.recovery_required} onclick={()=>act('reapply')}>{t("Повторить применение сохранённой группы")}</button>
    {/if}
    {#if job.message||job.message_code}<p role={['failed','uncertain'].includes(job.phase)?'alert':'status'}>{archiveMessage(job,presentation.language)}</p>{#if archiveDiagnostic(job,presentation.language)}<details><summary>{t('Техническая диагностика')}</summary><pre>{archiveDiagnostic(job,presentation.language)}</pre></details>{/if}{/if}
    {#if cancellable}<button onclick={()=>act('cancel')}>{job.mode==='reset'?"" + t("Отменить подготовку сброса") + "":"" + t("Отменить передачу группы") + ""}</button>{/if}
  {/if}
  <label>{t("ID операции группы")}<input bind:value={recoveryId} maxlength="32" spellcheck="false" disabled={active||!!pending}/></label>
  <label>{t("Устройство для чтения результата")}<input bind:value={recoveryDev} placeholder={device} disabled={active||!!pending}/></label>
  <label>{t("Тип операции")}<select aria-label="{t("Тип операции")}" bind:value={recoveryMode} disabled={active||!!pending}><option value="import">{t("Импорт")}</option><option value="export">{t("Экспорт")}</option><option value="reset">{t("Полный сброс Fan + Lighting")}</option></select></label>
  <div class="buttons">
    <button disabled={active||!connected||!recoveryValid} onclick={()=>act('recover')}>{t("Прочитать результат группы")}</button>
    {#if pending||job}<button disabled={active||job?.phase==='prepared'} onclick={()=>act('dismiss')}>{t("Закрыть сведения о группе")}</button>{/if}
    {#if pending&&!job&&pending.mode!=='reset'}<button disabled={active||!connected} onclick={()=>act('cancel')}>{t("Отменить сохранённую передачу")}</button>{/if}
  </div>
  {#if pending}<p>{t("ID сохранён для восстановления:")} <code>{pending.id}</code>{t(". Закрытие сведений не отменяет уже принятую операцию.")}</p>{/if}
  {#if error}<p role="alert">{archiveMessage(error,presentation.language)}</p>{#if archiveDiagnostic(error,presentation.language)}<details><summary>{t('Техническая диагностика')}</summary><pre>{archiveDiagnostic(error,presentation.language)}</pre></details>{/if}{/if}
</details>
<style>
  pre{white-space:pre-wrap;overflow-wrap:anywhere;user-select:text}
  details{border:1px solid #33445b;border-radius:12px;padding:18px;margin-block:16px}summary{cursor:pointer;font-weight:600}p{line-height:1.5}label{display:grid;gap:6px;margin:12px 0}.buttons{display:flex;flex-wrap:wrap;gap:8px}progress{width:100%}[role=alert]{color:#ffb7c6}code{user-select:all;overflow-wrap:anywhere}
</style>
