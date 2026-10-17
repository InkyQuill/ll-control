<script lang="ts">
  import {t} from './i18n.svelte';
  import {ValidationError} from './validation-error';
  import LightingPreview from './LightingPreview.svelte';
  import TemperatureInput from './TemperatureInput.svelte';
  import {presentation} from './presentation.svelte';
  import {temperatureSuffix} from './temperature';
  const displayUnit=$derived(presentation.settings?.temperature_unit??'celsius');
  const displaySuffix=$derived(temperatureSuffix(displayUnit));
  import { untrack } from 'svelte';
  import DynamicRgb from './DynamicRgb.svelte';
  import { lightingColorCount } from './lighting-palette';
  import { rpc, message, rgb, hex, type Sensor } from './api';
  let { device, connected, sensors }: { device: string; connected: boolean; sensors: Sensor[] } = $props();
  interface FanTopology {sync_to_pump:boolean;size:'280'|'360';manual_led_count:number}
  interface TopologyDraft {sync_to_pump:boolean;size:'280'|'360';manual_led_count:number|undefined}
  let topologySupported=$state(false);
  let fanSync=$state(false),fanSize=$state<'280'|'360'>('360'),manualLedCount=$state<number|undefined>(24);
  const topologyDrafts=new Map<string,TopologyDraft>();
  const topologyValid=$derived(Number.isInteger(manualLedCount) && manualLedCount!==undefined && manualLedCount>=8 && manualLedCount<=50);
  function currentTopology():FanTopology {if(!topologyValid)throw new ValidationError('Число LED должно быть целым от 8 до 50');return {sync_to_pump:fanSync,size:fanSize,manual_led_count:manualLedCount!};}
  function setTopology(value:TopologyDraft) {fanSync=value.sync_to_pump;fanSize=value.size;manualLedCount=value.manual_led_count;}
  interface LightingConfig {
    dynamic_mode?:boolean;
    fan_topology?:FanTopology|null;
    mode:string;color:{r:number;g:number;b:number};additional_colors?:{r:number;g:number;b:number}[];
    brightness_percent:number;speed_percent:number;direction:string;scope:string|null;disabled:boolean;
  }
  interface ModeControls { mode:string;color_count:number;speed:boolean;brightness:boolean;direction:boolean;defaults:LightingConfig }
  interface LightingInfo { dynamic_mode_supported?:boolean;fan_topology?:FanTopology|null;modes:string[];base?:LightingConfig|null;catalog?:ModeControls[];profiles?:LightingConfig[] }
  let catalog=$state<ModeControls[]>([]), savedModes=$state<LightingConfig[]>([]);
  let dynamicSupported=$state(false),useDynamic=$state(false);
  const controls=$derived(catalog.find(entry=>entry.mode===mode));
  const drafts=new Map<string,LightingConfig>(), selectedModes=new Map<string,string>();
  let draftOwner:{dev:string;kind:'pump'|'fan';mode:string}|null=null;
  const zoneKey=(dev:string,target:string)=>JSON.stringify([dev,target]);
  const modeKey=(dev:string,target:string,selected:string)=>JSON.stringify([dev,target,selected]);
  const normalize=(value:string)=>value==='static_color'?'static':value;
  interface Rule { sensor_id: string; threshold_c: number; color: { r: number; g: number; b: number } }
  interface Config { cpu: Rule | null; gpu: Rule | null }
  interface Thermal { requested: Config; active_source: string | null; degraded_sources: string[]; grace_remaining_ms: number; blocked_reason: string | null; fan: { last_error: string | null }; pump: { last_error: string | null } }
  let kind = $state<'pump' | 'fan'>('pump'), modes = $state<string[]>([]), mode = $state('');
  let color = $state('#7cdcca'), brightness = $state(100), speed = $state(50), direction = $state('forward'), scope = $state('outer'), disabled = $state(false);
  let additionalColors=$state(['#0000ff','#00ff00','#ffff00']);
  const colorCount=$derived(controls?.color_count ?? lightingColorCount(kind,mode));
  let error = $state<ValidationError|string|null>(null), thermalBaseMissing = $state(false), notice = $state(''), busy = $state(false), thermal = $state<Thermal | null>(null), loaded = $state(false);
  let rules = $state([{ source: 'cpu' as const, enabled: false, sensor: '', threshold: 85, color: '#ff4c4c' }, { source: 'gpu' as const, enabled: false, sensor: '', threshold: 85, color: '#ff9944' }]);
  let dynamicBusy = $state(false), loadingBase=$state(false);
  let draftRevision=0;
  let controlsBusy = $derived(busy || dynamicBusy);
  let generation = 0, revision = 0;
  let rulesDevice = $state(''), pollError = $state('');
  let rulesCurrent = $derived(loaded && rulesDevice === device);
  let temperatures = $derived(sensors.filter(s => s.unit === 'celsius'));
  $effect(() => {
    const dev = device, online = connected, target = kind;
    untrack(cacheMode);draftOwner=null;
    const initialDraft=draftRevision;
    generation++; revision++; modes = []; catalog=[];savedModes=[];topologySupported=false;dynamicSupported=false; thermal = null; pollError = ''; notice = ''; error = null; thermalBaseMissing=false;
    if (!dev || !online) {draftOwner=null;return;}
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    void rpc<LightingInfo>('get_lighting_info', { dev, kind: target }).then(info => {
      if(cancelled)return;
      modes=info.modes;catalog=info.catalog ?? [];savedModes=info.profiles ?? [];dynamicSupported=info.dynamic_mode_supported===true;topologySupported=target==='fan'&&!!info.fan_topology;
      if(info.base)savedModes=[...savedModes.filter(c=>normalize(c.mode)!==normalize(info.base!.mode)),info.base];
      const previous=selectedModes.get(zoneKey(dev,target));
      const desired=previous&&modes.includes(previous)?previous:catalog.length&&info.base&&modes.includes(normalize(info.base.mode))?normalize(info.base.mode):catalog.length?modes[0]??'':modes.includes(mode)?mode:modes.find(m=>m==='static_color')??modes[0]??'';
      if(initialDraft!==draftRevision){mode=desired;draftOwner={dev,kind:target,mode:desired};return;}
      if(target==='fan')setTopology(topologyDrafts.get(dev) ?? info.fan_topology ?? {sync_to_pump:false,size:'360',manual_led_count:24});
      restoreMode(dev,target,desired);
    }).catch(e => { if (!cancelled) error = e instanceof ValidationError?e:message(e); });
    async function refresh() {
      const started = revision;
      try { if (!controlsBusy) { const result = await rpc<Thermal>('get_thermal_state', { dev }); if (!cancelled && started === revision) { thermal = result; pollError = ''; } } }
      catch (e) { if (!cancelled && started === revision) { thermal = null; pollError = message(e); } }
      if (!cancelled) timer = setTimeout(refresh, 1500);
    }
    timer = setTimeout(refresh, 0); return () => { cancelled = true; clearTimeout(timer); };
  });
  function markDraftChange() {if(draftOwner?.dev===device && draftOwner.kind===kind)draftRevision++;}
  function cacheMode() {
    if(!draftOwner)return;
    const {dev,kind:target,mode:selected}=draftOwner;
    if(target==='fan')topologyDrafts.set(dev,{sync_to_pump:fanSync,size:fanSize,manual_led_count:manualLedCount});
    const cfg:LightingConfig={...(target==='pump'&&dynamicSupported?{dynamic_mode:useDynamic}:{}),mode:selected,color:toRgb(color),additional_colors:additionalColors.slice(0,Math.max(0,(lightingColorCount(target,selected) ?? 1)-1)).map(toRgb),brightness_percent:brightness,speed_percent:speed,direction,scope:target==='pump'?scope:null,disabled};
    drafts.set(modeKey(dev,target,selected),cfg);selectedModes.set(zoneKey(dev,target),selected);
  }
  function setDraft(dev:string,target:'pump'|'fan',cfg:LightingConfig,selected=cfg.mode) {
    const asHex=(c:{r:number;g:number;b:number})=>hex([c.r,c.g,c.b]);
    mode=selected;color=asHex(cfg.color);useDynamic=cfg.dynamic_mode===true;
    additionalColors=Array.from({length:3},(_,index)=>asHex(cfg.additional_colors?.[index] ?? {r:0,g:0,b:0}));
    brightness=cfg.brightness_percent;speed=cfg.speed_percent;direction=cfg.direction;scope=cfg.scope ?? 'inner';disabled=cfg.disabled;
    draftOwner={dev,kind:target,mode:selected};cacheMode();
  }
  function restoreMode(dev:string,target:'pump'|'fan',selected:string) {
    const cfg=drafts.get(modeKey(dev,target,selected)) ?? savedModes.find(c=>normalize(c.mode)===normalize(selected)) ?? catalog.find(c=>c.mode===selected)?.defaults;
    if(cfg)setDraft(dev,target,cfg,selected);
    else {mode=selected;useDynamic=false;draftOwner={dev,kind:target,mode:selected};}
  }
  function policyChanged(dev:string,selected:string,enabled:boolean) {
    const canonical=normalize(selected), key=modeKey(dev,'pump',canonical);
    const retained=drafts.get(key);
    if(retained)drafts.set(key,{...retained,dynamic_mode:enabled});
    if(dev!==device)return;
    if(kind==='pump')savedModes=savedModes.map(cfg=>normalize(cfg.mode)===canonical?{...cfg,dynamic_mode:enabled}:cfg);
    if(kind==='pump'&&normalize(mode)===canonical){useDynamic=enabled;draftRevision++;}
  }
  function changeMode() {const selected=mode;cacheMode();restoreMode(device,kind,selected);draftRevision++;}
  function resetMode() {
    if(!controls || controlsBusy || loadingBase)return;
    setDraft(device,kind,controls.defaults);draftRevision++;
    notice='Значения по умолчанию загружены в черновик.';
  }
  async function loadBase() {
    if(controlsBusy || loadingBase || !device || !connected)return;
    const started=generation, draft=draftRevision, dev=device, target=kind;
    loadingBase=true;error=null;thermalBaseMissing=false;notice='';
    try {
      const info=await rpc<LightingInfo>('get_lighting_info',{dev,kind:target});
      if(started!==generation)return;
      if(draft!==draftRevision){notice='Черновик изменён; загрузка не применена.';return;}
      const base=info.base;
      if(!base){notice='Настройка режима для загрузки отсутствует. Черновик сохранён.';return;}
      const selected=info.modes.includes(base.mode)?base.mode:base.mode==='static_color'&&info.modes.includes('static')?'static':null;
      if(!selected)throw new ValidationError('Сохранённый режим не поддерживается редактором. Черновик сохранён.');
      cacheMode();modes=info.modes;catalog=info.catalog ?? [];savedModes=info.profiles ?? [];dynamicSupported=info.dynamic_mode_supported===true;topologySupported=target==='fan'&&!!(info.fan_topology??base.fan_topology);
      setDraft(dev,target,base,selected);
      if(target==='fan' && (info.fan_topology ?? base.fan_topology)){setTopology((info.fan_topology ?? base.fan_topology)!);cacheMode();}
      draftRevision++;
      notice='Настройки зоны загружены. Цвет могут временно менять оповещения и привязки к датчикам.';
    }catch(e){if(started===generation)error=e instanceof ValidationError?e:message(e);}
    finally{loadingBase=false;}
  }
  function toRgb(value: string) { const [r,g,b] = rgb(value); return { r,g,b }; }
  async function action(run: () => Promise<void>, thermalAction = false) {
    if (controlsBusy) return;
    const started = generation; revision++; busy = true; error = null; thermalBaseMissing=false; notice = '';
    try { await run(); }
    catch (e) { if (started === generation) { error = e instanceof ValidationError?e:message(e); thermalBaseMissing = thermalAction && !(e instanceof ValidationError) && message(e).includes('thermal alerts require known fan and pump lighting bases'); } }
    finally { revision++; busy = false; }
  }
  function loadRules() {
    if (!thermal) return;
    rules = rules.map(r => { const saved = thermal!.requested[r.source]; return { ...r, enabled: !!saved, sensor: saved?.sensor_id ?? '', threshold: saved?.threshold_c ?? 85, color: saved ? hex([saved.color.r,saved.color.g,saved.color.b]) : r.color }; });
    loaded = true; rulesDevice = device;
  }
  async function applyRules() {
    if (!rulesCurrent) throw new ValidationError('Загрузите оповещения выбранного устройства перед применением');
    const dev = device, started = generation;
    const config: Config = { cpu: null, gpu: null };
    for (const r of rules) {
      if (r.enabled) { if (!r.sensor) throw new ValidationError('Для каждого включённого оповещения нужен датчик'); if (!Number.isFinite(r.threshold) || r.threshold < 0 || r.threshold > 150) throw new ValidationError('Порог оповещения должен быть числом от 0 до 150 °C'); config[r.source] = { sensor_id: r.sensor, threshold_c: r.threshold, color: toRgb(r.color) }; }
    }
    const result = await rpc<Thermal>('set_thermal_config', { dev, config });
    if (started !== generation) return;
    revision++; thermal = result;
    const failure = result.blocked_reason ?? result.fan.last_error ?? result.pump.last_error;
    if (failure) throw new Error(failure);
    notice = 'Температурные оповещения сохранены.';
  }
</script>
<section aria-label="{t("Подсветка")}">
  <div class="lighting-workspace"><LightingPreview {kind} {color} {brightness} {disabled} {colorCount}/><div class="lighting-controls">
  <h2>{t("Черновик настройки RGB")}</h2>
  <button disabled={controlsBusy || loadingBase || !connected || !device} onclick={loadBase}>{t("Загрузить настройки подсветки")}</button>
  <fieldset class="manual" disabled={!modes.length} oninput={markDraftChange} onchange={markDraftChange}>
  <legend>{t("Настройки выбранной зоны")}</legend>
  <div class="row"><label>{t("Зона")}<select bind:value={kind} disabled={controlsBusy}><option value="pump">{t("Кольцо помпы")}</option><option value="fan">{t("Вентиляторы LCDV2")}</option></select></label><label>{t("Эффект")}<select bind:value={mode} onchange={changeMode} disabled={!modes.length}>{#each modes as effect}<option value={effect}>{effect}</option>{/each}</select></label></div>
  {#if kind==='pump'&&dynamicSupported}
    <label>{t("Управление подсветкой")}<select bind:value={useDynamic} disabled={controlsBusy}>
      <option value={false}>{t("Обычная настройка")}</option><option value={true}>{t("По сохранённому датчику")}</option>
    </select></label>
    {#if useDynamic}<p>{t("При применении восстановятся последний датчик и его параметры для выбранного эффекта. Сначала настройте привязку в разделе «Подсветка помпы по датчику».")}</p>{/if}
  {/if}
  <div class="row">{#if colorCount !== 0}<label>{t("Цвет RGB")}<input type="color" bind:value={color}/></label>{/if}
    {#each additionalColors.slice(0,Math.max(0,(colorCount ?? 1)-1)) as _,index (index)}<label>{t("Цвет RGB")} {index+2}<input type="color" bind:value={additionalColors[index]} disabled={controlsBusy}/></label>{/each}{#if controls?.brightness !== false}<label>{t("Яркость RGB:")} {brightness}%<input type="range" min="0" max="100" bind:value={brightness}/></label>{/if}{#if controls?.speed !== false}<label>{t("Скорость:")} {speed}%<input type="range" min="0" max="100" bind:value={speed}/></label>{/if}</div>
  <div class="row">{#if controls?.direction !== false}<label>{t("Направление")}<select bind:value={direction}><option value="forward">{t("Вперёд")}</option><option value="reverse">{t("Назад")}</option></select></label>{/if}{#if kind === 'pump'}<label>{t("Контур")}<select bind:value={scope}><option value="outer">{t("Внешний")}</option><option value="inner">{t("Внутренний")}</option><option value="all">{t("Все")}</option></select></label>{/if}<label><input type="checkbox" bind:checked={disabled}/>{t("Выключить подсветку")}</label></div>
  {#if kind==='fan'&&topologySupported}
    <fieldset><legend>{t("Синхронизация и число LED")}</legend>
      <label><input type="checkbox" bind:checked={fanSync} disabled={controlsBusy}/>{t("Синхронизировать вентиляторы с помпой")}</label>
      {#if fanSync}<label>{t("Размер комплекта вентиляторов")}<select bind:value={fanSize} disabled={controlsBusy}><option value="280">{t("280 мм")}</option><option value="360">{t("360 мм")}</option></select></label>{/if}
      <label>{t("Число LED в ручном режиме")}
        <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK needs explicit spinbutton for native Value.) -->
        <input type="number" role="spinbutton" min="8" max="50" step="1" aria-valuemin="8" aria-valuemax="50" aria-valuenow={Number.isFinite(manualLedCount)?manualLedCount:undefined} aria-invalid={!topologyValid} bind:value={manualLedCount} disabled={controlsBusy}/></label>
      {#if fanSync}<p>{t("При синхронизации используются")} {fanSize==='280'?16:24} {t("LED. Ручное значение сохранится для отключения синхронизации.")}</p>{/if}
      {#if !topologyValid}<p role="status">{t("Введите целое число LED от 8 до 50 перед применением.")}</p>{/if}
    </fieldset>
  {/if}
  {#if controls}<button disabled={controlsBusy || loadingBase} onclick={resetMode}>{t("Значения режима по умолчанию")}</button>{/if}
  <button class="primary" disabled={controlsBusy || loadingBase || !connected || !device || !mode || kind==='fan'&&topologySupported&&!topologyValid} onclick={() => action(async () => { const started = generation; cacheMode();await rpc('set_lighting', { dev: device, kind, cfg: { ...(kind==='pump'&&dynamicSupported?{dynamic_mode:useDynamic}:{}),fan_topology:kind==='fan'&&topologySupported?currentTopology():null, mode, color: toRgb(color), additional_colors: additionalColors.slice(0,Math.max(0,(colorCount ?? 1)-1)).map(toRgb), brightness_percent: brightness, speed_percent: controls?.speed===false?0:speed, direction:controls?.direction===false?'forward':direction, scope: kind === 'pump' ? scope : null, disabled } }); if (started !== generation) return; notice = dynamicSupported&&useDynamic&&kind==='pump'?'Режим по сохранённому датчику восстановлен. Фактическое состояние показано в разделе ниже.':'Команда подсветки выполнена. Thermal grace после ручного изменения — 10 секунд.'; })}>{t("Применить подсветку")}</button>
  </fieldset>
  </div></div>
  <h2>{t("Температурные оповещения")}</h2>
  <p>{t("Перед включением примените обычную подсветку в обеих зонах — помпы и вентиляторов LCDV2. Daemon восстановит эти настройки после оповещения.")}</p>
  <p>{t("Активный источник:")} {thermal?.active_source ?? '—'} · Grace: {thermal ? Math.ceil(thermal.grace_remaining_ms / 1000) : '—'} {t("s. Недоступные источники:")} {thermal?.degraded_sources.join(', ') || '—'}.</p>
  {#if thermal?.blocked_reason}<p role="alert">{thermal.blocked_reason}</p>{/if}
  <button disabled={controlsBusy || !thermal} onclick={loadRules}>{t("Загрузить текущие оповещения")}</button>
  {#if loaded && !rulesCurrent}<p role="alert">{t("Черновик оповещений относится к другому устройству. Загрузите настройки выбранного устройства перед применением.")}</p>{/if}
  {#if loaded}{#each rules as rule}<fieldset><legend>{rule.source.toUpperCase()}</legend><label><input type="checkbox" bind:checked={rule.enabled}/>{t("Включить")} {rule.source.toUpperCase()}</label><div class="row"><label>{t("Датчик")} {rule.source.toUpperCase()}<select bind:value={rule.sensor} disabled={!rule.enabled}><option value="">{t("Выберите источник")}</option>{#each temperatures as sensor (sensor.id)}<option value={sensor.id}>{sensor.label} · {sensor.status}</option>{/each}{#if rule.sensor && !temperatures.some(s => s.id === rule.sensor)}<option value={rule.sensor}>{t("Недоступен:")} {rule.sensor}</option>{/if}</select></label><label>{t("Порог")} {rule.source.toUpperCase()}, {displaySuffix}
      <TemperatureInput bind:value={rule.threshold} min={0} max={150} disabled={!rule.enabled}/></label><label>{t("Цвет оповещения")} {rule.source.toUpperCase()}<input type="color" bind:value={rule.color} disabled={!rule.enabled}/></label></div></fieldset>{/each}{:else}<p>{t("Загрузите текущие правила перед редактированием. Оповещения временно перекрывают обе RGB-зоны и затем восстанавливают сохранённую подсветку.")}</p>{/if}
  <button disabled={!rulesCurrent || controlsBusy || !device || !connected} onclick={() => action(applyRules, true)}>{t("Применить оповещения")}</button>
  {#if pollError}<p role="alert">{t('Не удалось прочитать состояние оповещений:')} <span>{pollError}</span></p>{/if}
  {#if error}<p class="error" role="alert">{#if error instanceof ValidationError}{t(error.resource,...error.values)}{:else}{t('Ошибка подсветки:')} <span>{error}</span>{#if thermalBaseMissing} {t('Сначала примените обычную подсветку в обеих зонах: кольцо помпы и вентиляторы LCDV2. Эти настройки нужны для восстановления после оповещения.')}{/if}{/if}</p>{/if}{#if notice}<p role="status">{t(notice)}</p>{/if}
  <DynamicRgb {device} {connected} {sensors} disabled={busy} bind:busy={dynamicBusy} onpolicychange={policyChanged}/>
</section>
<style>.lighting-workspace{display:grid;grid-template-columns:minmax(260px,1fr) minmax(300px,1fr);gap:32px}.lighting-controls{min-width:0}.lighting-controls>button{margin-bottom:16px}@media(max-width:1100px){.lighting-workspace{grid-template-columns:1fr}}.manual{display:grid;gap:18px}.manual .row{flex-wrap:wrap}section{max-width:none;display:grid;gap:18px}.row{display:flex;gap:15px;align-items:end}h2{font-size:18px;margin-top:8px}button{justify-self:start}p{color:var(--text-muted);line-height:1.6}fieldset{border:1px solid var(--border);border-radius:10px;padding:15px;display:grid;gap:15px}input[type=color]{max-width:120px}input[type=checkbox]{justify-self:start}.error,[role=alert]{color:var(--danger)}[role=status]{color:var(--success)}</style>
