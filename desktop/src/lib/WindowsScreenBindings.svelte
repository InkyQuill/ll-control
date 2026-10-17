<script lang="ts">
 import {t} from './i18n.svelte';
 import {rpc,message,type Sensor,type PreparedClipInfo} from './api';
 import type {FontChoice} from './font-upload';
 import type {GroupStatus} from './group-archive';
 import {screenFonts,screenOptions,restoreScreenOptions,type ScreenOptions} from './windows-screen';
 import {emptyWindowsIds,windowsBindingInvalid,windowsSourceUnverified,windowsRoleLabel,windowsUnit,windowsUnitLabel} from './windows-control';
 let {job,sensors,fonts=[],editable,oninspect,onprepare}:{job:GroupStatus;sensors:Sensor[];fonts?:FontChoice[];editable:boolean;oninspect:(record:number)=>void;onprepare:(options:ScreenOptions)=>void}=$props();
 let record=$state(''),name=$state(''),ids=$state(emptyWindowsIds()),fontMap=$state<Record<string,string>>({}),resources=$state<Record<string,string>>({});
 let images=$state<{key:string;name:string|null}[]>([]),videos=$state<PreparedClipInfo[]>([]),loading=$state(false),error=$state('');
 let restored='',requirementsFor='';
 const requirements=$derived(job.windows_screen_requirements);
 const canEdit=$derived(editable&&job.phase==='bindings');
 const draft=$derived(screenOptions(requirements,record,name,ids,fontMap,resources,sensors,images.map(v=>v.key),videos.map(v=>v.key),fonts.map(v=>v.key)));
 $effect(()=>{
  const source=requirements;
  const key=source?`${job.id}:${source.record_index}:${source.backup_sha256}`:'';
  if(key&&key!==requirementsFor){requirementsFor=key;ids=emptyWindowsIds();fontMap={};resources={};}
  const options=job.windows_screen_options,encoded=options?JSON.stringify(options):'';
  if(options&&encoded!==restored){restored=encoded;const value=restoreScreenOptions(options);record=String(options.record);name=value.name;ids={...emptyWindowsIds(),...value.ids};fontMap=value.fonts;resources=value.resources;}
 });
 function resourceValue(reference:string){return Object.hasOwn(resources,reference)?resources[reference]:'';}
 async function refresh(){
  if(loading||!canEdit)return;loading=true;error='';
  try{const [a,b]=await Promise.all([rpc<{key:string;name:string|null}[]>('list_named_theme_assets'),rpc<PreparedClipInfo[]>('list_prepared_clips')]);images=a;videos=b;}
  catch(e){error=message(e);}finally{loading=false;}
 }
</script>
<section class="screen-bindings" aria-label={t('Привязки Windows Screen')}>
 <h3>{t('Исходный экран Windows')}</h3>
 <label>{t('Исходный контроллер Windows')}<select aria-label={t('Исходный контроллер Windows')} bind:value={record} disabled={!canEdit}><option value="">{t('Выберите record контроллера')}</option>{#each job.windows_source?.controllers??[] as source (source.record_index)}<option value={String(source.record_index)}>{t('Record {0} · {1}',source.record_index,source.profile_sha256.slice(0,12))}</option>{/each}</select></label>
 <button disabled={!canEdit||record===''} onclick={()=>oninspect(Number(record))}>{t('Прочитать требования экрана')}</button>
 {#if requirements&&record===String(requirements.record_index)}
  {#if requirements.deferred.length}<p role="alert">{t('Этот экран пока нельзя импортировать: текущая сцена, эффект или файлы требуют дополнительной поддержки.')}</p>{/if}
  <p>{t('Назначьте источники явно. Неизвестные ID датчиков сохраняются без подмены; до Apply экран не меняется.')}</p>
  <label>{t('Название импортированной темы')}<input bind:value={name} maxlength="128" disabled={!canEdit}/></label>
  <div class="fields">{#each requirements.bindings.sensor_roles as role (role)}
   <div><label>{windowsRoleLabel(role)} ({windowsUnitLabel(role)})<input list={`screen-${job.id}-${role}`} bind:value={ids[role]} disabled={!canEdit} maxlength="1024" aria-invalid={windowsBindingInvalid(role,ids,sensors)} placeholder={t('Выберите или введите ID датчика')}/></label>
   <datalist id={`screen-${job.id}-${role}`}>{#each sensors.filter(s=>s.unit===windowsUnit(role)) as sensor (sensor.id)}<option value={sensor.id}>{sensor.label}</option>{/each}</datalist>
   {#if windowsBindingInvalid(role,ids,sensors)}<p role="alert">{t('ID повторяется, имеет неподходящие единицы или недопустимый формат.')}</p>{:else if windowsSourceUnverified(role,ids,sensors)}<p>{t('Источник недоступен или не подтверждён. ID сохранён без подмены.')}</p>{/if}</div>
  {/each}</div>
  {#each requirements.bindings.fonts as family (family)}<label>{t('Шрифт для {0}',family)}<select aria-label={t('Шрифт для {0}',family)} value={fontMap[family]??''} disabled={!canEdit} onchange={e=>fontMap={...fontMap,[family]:e.currentTarget.value}}><option value="">{t('Выберите шрифт')}</option>{#each screenFonts as [value,label]}<option {value}>{label}</option>{/each}{#each fonts as font (font.key)}<option value={`asset:${font.key}`}>{font.name}</option>{/each}{#if fontMap[family]?.startsWith('asset:')&&!fonts.some(f=>`asset:${f.key}`===fontMap[family])}<option value={fontMap[family]}>{fontMap[family]}</option>{/if}</select></label>{/each}
  <p>{t('Шрифты выбираются явно; совпадение с Windows по метрикам ещё требует проверки.')}</p>
  {#if Object.keys(requirements.bindings.resources).length}<button disabled={!canEdit||loading} onclick={refresh}>{t('Обновить доступные изображения и видео')}</button>{/if}
  {#each Object.entries(requirements.bindings.resources) as [reference,imageOnly] (reference)}
   <label><span class="reference">{reference}</span><span>{imageOnly?t('Требуется изображение'):t('Изображение или видео')}</span><select aria-label={reference} value={resourceValue(reference)??''} disabled={!canEdit} onchange={e=>resources={...resources,[reference]:e.currentTarget.value}}><option value="">{t('Выберите ресурс')}</option>
    {#each requirements.payloads.filter(p=>p.kind==='image'||!imageOnly&&p.kind==='video') as payload (payload.entry)}<option value={`archive:${payload.entry}`}>{t('Из архива: {0}',payload.entry)}</option>{/each}
    {#each images as image (image.key)}<option value={`image:${image.key}`}>{t('Изображение: {0}',image.name??image.key)}</option>{/each}
    {#if !imageOnly}{#each videos as video (video.key)}<option value={`video:${video.key}`}>{t('Видео: {0}',video.name)}</option>{/each}{/if}
    {#if resourceValue(reference)&&!resourceValue(reference).startsWith('archive:')&&!images.some(i=>`image:${i.key}`===resourceValue(reference))&&!videos.some(v=>`video:${v.key}`===resourceValue(reference))}<option value={resourceValue(reference)}>{resourceValue(reference)}</option>{/if}
   </select></label>
  {/each}
  {#if error}<p role="alert">{t('Не удалось прочитать ресурсы: {0}',error)}</p>{/if}
  <p>{t('При подготовке все файлы будут проверены и декодированы. Сохранённые Linux-профили останутся без изменений.')}</p>
  {#if job.phase==='bindings'}<button disabled={!canEdit||!draft} onclick={()=>{if(draft)onprepare(draft);}}>{t('Подготовить импорт экрана')}</button>{/if}
 {/if}
</section>
<style>
 .screen-bindings{border-top:1px solid var(--border);padding-top:20px;margin-top:20px}.fields{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:0 18px}label{display:grid;gap:6px;margin:14px 0}.reference{overflow-wrap:anywhere;user-select:text}p{line-height:1.5;color:var(--text-muted)}[role=alert]{color:var(--danger)}@media(max-width:800px){.fields{grid-template-columns:1fr}}
</style>
