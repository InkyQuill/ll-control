<script lang="ts">
  import {t} from './i18n.svelte';
  import TemperatureInput from './TemperatureInput.svelte';
  import {presentation} from './presentation.svelte';
  import {temperatureSuffix} from './temperature';
  const displayUnit=$derived(presentation.settings?.temperature_unit??'celsius');
  const displaySuffix=$derived(temperatureSuffix(displayUnit));
  import { tick } from 'svelte';
  import Profiles from './Profiles.svelte';
  import { rpc, message, coolingPreset, type Sensor, type CoolingPolicy, type CoolingState } from './api';
  let { device, sensors, connected }: { device: string; sensors: Sensor[]; connected: boolean } = $props();
  let target = $state<'pump' | 'fan'>('pump');
  let mode = $state<CoolingPolicy['mode']>('fixed');
  let percent = $state(80), rpm = $state(2200), sensorId = $state(''), hysteresis = $state(2), fallback = $state(100);
  let editor: HTMLElement | undefined;
  let points = $state([{ celsius: 30, value: 40 }, { celsius: 70, value: 100 }]);
  let cooling = $state<CoolingState | null>(null), error = $state(''), notice = $state(''), busy = $state(false);
  let revision = 0, generation = 0;
  let pollError = $state('');
  let current = $derived(cooling?.[target]);
  let curve = $derived(['curve', 'fan_curve', 'curve_rpm'].includes(mode));
  let rpmMode = $derived(mode === 'fixed_rpm' || mode === 'curve_rpm');
  let minDuty = $derived(target === 'pump' ? 20 : 10);
  let fixedValue = $derived(rpmMode ? rpm : percent);
  let fixedMin = $derived(rpmMode ? 800 : minDuty);
  let fixedMax = $derived(rpmMode ? 3600 : 100);
  let fixedInvalid = $derived(!Number.isInteger(fixedValue) || fixedValue < fixedMin || fixedValue > fixedMax);
  let temperatures = $derived(sensors.filter(s => s.unit === 'celsius'));
  let graph = $derived(points.map(p => `${(Math.max(-50, Math.min(150, p.celsius)) + 50) * 1.5},${120 - Math.max(0, Math.min(1, p.value / (rpmMode ? 3600 : 100))) * 120}`).join(' '));
  $effect(() => {
    const dev = device, online = connected;
    generation++; revision++; cooling = null; pollError = ""; notice = ""; error = "";
    if (!dev || !online) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      const started = revision;
      try { if (!busy) { const result = await rpc<CoolingState>('get_cooling_state', { dev }); if (!cancelled && started === revision) { cooling = result; pollError = ''; } } }
      catch (e) { if (!cancelled && started === revision) { cooling = null; pollError = message(e); } }
      if (!cancelled) timer = setTimeout(refresh, 1500);
    }
    timer = setTimeout(refresh, 0);
    return () => { cancelled = true; clearTimeout(timer); };
  });
  async function removePoint(index: number) {
    if (points.length <= 2 || index < 0 || index >= points.length) return;
    const draft = points, selected = target, selectedMode = mode, owner = editor, started = generation;
    points.splice(index, 1);
    await tick();
    if (!owner?.isConnected || owner !== editor || started !== generation || draft !== points || selected !== target || selectedMode !== mode || !curve) return;
    const fields = owner.querySelectorAll<HTMLInputElement>('[data-curve-temperature]');
    fields[Math.min(index, fields.length - 1)]?.focus();
  }
  function load(policy: CoolingPolicy) {
    mode = policy.mode;
    if ('percent' in policy) percent = policy.percent;
    if ('rpm' in policy) rpm = policy.rpm;
    if ('sensor_id' in policy) {
      sensorId = policy.sensor_id; hysteresis = policy.hysteresis_c; fallback = policy.fallback_percent;
      points = policy.points.map(p => ({ celsius: p.celsius, value: 'rpm' in p ? p.rpm : p.percent }));
    }
  }
  function draftKey() { return JSON.stringify({mode,percent,rpm,sensorId,hysteresis,fallback,points}); }
  async function preset(name: string) {
    const started = generation, draft = draftKey();
    busy = true; error = ''; notice = '';
    try {
      const result = await coolingPreset(target, name, sensorId);
      if (started !== generation) return;
      if (draft !== draftKey()) { notice = 'Черновик изменился. Preset не заменил ваши правки.'; return; }
      load(result); notice = 'Preset загружен в редактор. Нажмите «Применить охлаждение» для отправки.';
    }
    catch (e) { if (started === generation) error = message(e); } finally { busy = false; }
  }
  function changeMode() {
    points = mode === 'curve_rpm' ? [{ celsius: 30, value: 2200 }, { celsius: 70, value: 3600 }] : [{ celsius: 30, value: 40 }, { celsius: 70, value: 100 }];
  }
  function policy(): CoolingPolicy {
    if (!curve && fixedInvalid) throw new Error(`Введите целое значение ${fixedMin}–${fixedMax}`);
    if (mode === 'fixed' || mode === 'motherboard') return { mode, percent };
    if (mode === 'fixed_rpm') return { mode, rpm };
    if (!sensorId) throw new Error('Выберите температурный датчик');
    if (!Number.isFinite(hysteresis) || hysteresis < 0 || hysteresis > 10) throw new Error('Гистерезис должен быть от 0 до 10 °C');
    if (!Number.isInteger(fallback) || fallback < minDuty || fallback > 100) throw new Error(`Fallback PWM должен быть целым от ${minDuty} до 100%`);
    if (points.some(p => p.celsius < -50 || p.celsius > 150 || (mode !== 'curve' && !Number.isInteger(p.celsius)) || !Number.isInteger(p.value) || p.value < fixedMin || p.value > fixedMax)) throw new Error('Проверьте диапазоны и целые значения точек кривой');
    if (points.length < 2 || points.length > 16 || points.some((p, i) => !Number.isFinite(p.celsius) || !Number.isInteger(p.value) || (i > 0 && (p.celsius <= points[i-1].celsius || p.value < points[i-1].value)))) throw new Error('Температуры должны возрастать, мощность не должна снижаться');
    const shared = { sensor_id: sensorId, hysteresis_c: hysteresis, fallback_percent: fallback };
    return mode === 'curve_rpm' ? { mode, ...shared, points: points.map(p => ({ celsius: p.celsius, rpm: p.value })) } : { mode, ...shared, points: points.map(p => ({ celsius: p.celsius, percent: p.value })) };
  }
  async function apply() {
    busy = true; error = ''; notice = '';
    const dev = device, selected = target, started = generation; revision++;
    try {
      const result = await rpc<CoolingState>('set_cooling_policy', { dev, target: selected, policy: policy() });
      if (started !== generation) return;
      revision++; cooling = result;
      if (result[selected].last_error) throw new Error(result[selected].last_error!);
      notice = 'Политика принята daemon. Applied PWM показан отдельно; реальные RPM — на странице датчиков.';
    } catch (e) { if (started === generation) error = message(e); } finally { revision++; busy = false; }
  }
</script>
<section aria-label="{t("Охлаждение")}" bind:this={editor}>
  <div class="row"><label>{t("Канал")}<select bind:value={target} onchange={() => { mode = 'fixed'; notice = ''; error = ''; }} disabled={busy}><option value="pump">{t("Помпа")}</option><option value="fan">{t("Вентиляторы LCDV2")}</option></select></label>
  <label>{t("Режим")}<select bind:value={mode} onchange={changeMode} disabled={busy}><option value="fixed">{t("Фиксированный PWM")}</option><option value={target === 'fan' ? 'fan_curve' : 'curve'}>{t("Температурная кривая PWM")}</option>{#if target === 'pump'}<option value="fixed_rpm">{t("Целевые RPM")}</option><option value="curve_rpm">{t("Температурная кривая RPM")}</option><option value="motherboard">{t("Источник motherboard")}</option>{/if}</select></label></div>
  {#key target}<Profiles resetSensor={sensorId} group={target} {connected} disabled={busy} draft={() => ({ kind: target, value: policy() })} onload={content => { if (content.kind === 'fan' || content.kind === 'pump') load(content.value); }}/>{/key}
  <article class="summary"><h2>{t("Состояние daemon ·")} {target === 'pump' ? "" + t("помпа") + "" : "" + t("вентиляторы") + ""}</h2><div class="row"><strong>Applied: {current?.applied_percent ?? '—'}%</strong><span>{current?.motherboard ? 'Motherboard' : 'MCU'}</span><span>{current?.fallback_active ? "" + t("Fallback активен") + "" : "" + t("Fallback не активен") + ""}</span></div>{#if current?.last_error}<p role="alert">{t(current.last_error)}</p>{/if}<button disabled={!current?.requested || busy} onclick={() => { if (current?.requested) load(current.requested); }}>{t("Загрузить текущую политику")}</button></article>
  {#if target === 'fan'}<p>{t("Этот канал управляет вентиляторами, подключёнными к LCDV2. Вентиляторы на материнской плате управляются отдельно.")}</p>{/if}
  <label>{t("Источник температуры")}<select bind:value={sensorId}><option value="">{t("Выберите датчик для кривой / preset")}</option>{#each temperatures as sensor}<option value={sensor.id}>{sensor.label} · {sensor.status}</option>{/each}{#if sensorId && !temperatures.some(s => s.id === sensorId)}<option value={sensorId}>{t("Недоступен:")} {sensorId}</option>{/if}</select></label>
  <div class="presets">{#each target === 'pump' ? ['quiet', 'performance'] : ['quiet', 'standard', 'high', 'full'] as name}<button disabled={!sensorId || busy} onclick={() => preset(name)}>{name}</button>{/each}</div>
  {#if !curve}<label>{rpmMode ? "" + t("Целевые RPM (reference mapping)") + "" : 'PWM, %'}
    <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK exposes number as textbox without Value; native AT-SPI regression verifies this workaround.) -->
    <input type="number" role="spinbutton" min={fixedMin} max={fixedMax} value={fixedValue} aria-valuemin={fixedMin} aria-valuemax={fixedMax} aria-valuenow={Number.isFinite(fixedValue) ? fixedValue : undefined} aria-invalid={fixedInvalid} oninput={e => { if (rpmMode) rpm = e.currentTarget.valueAsNumber; else percent = e.currentTarget.valueAsNumber; }} /></label>{:else}
    <svg viewBox="-20 -10 330 155" role="img" aria-label="{t("Черновик температурной кривой")}"><path d="M0 0V120H300" stroke="#677c98" fill="none"/><polyline points={graph} fill="none" stroke="#83e0c5" stroke-width="3"/>{#each points as p}<circle cx={(Math.max(-50, Math.min(150,p.celsius))+50)*1.5} cy={120-Math.max(0,Math.min(1,p.value/(rpmMode?3600:100)))*120} r="4" fill="#e3fff4"/>{/each}<text x="120" y="140" fill="#b7c9df" font-size="10">{t("Температура")} {displaySuffix}</text></svg>
    {#each points as p, i (p)}<div class="row"><label>{t("Точка")} {i+1}: {displaySuffix}
      <TemperatureInput curve bind:value={p.celsius} min={-50} max={150} step={mode==='curve'?'any':1} invalid={mode!=='curve'&&!Number.isInteger(p.celsius)||i>0&&p.celsius<=points[i-1].celsius}/></label><label>{rpmMode ? 'RPM' : 'PWM %'}
      <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires explicit spinbutton for the native Value interface; verified by AT-SPI.) -->
      <input type="number" role="spinbutton" bind:value={p.value} min={fixedMin} max={fixedMax} aria-valuemin={fixedMin} aria-valuemax={fixedMax} aria-valuenow={Number.isFinite(p.value) ? p.value : undefined} aria-invalid={!Number.isInteger(p.value) || p.value < fixedMin || p.value > fixedMax}/></label><button aria-label={`Удалить точку ${i+1}`} disabled={points.length <= 2} onclick={() => removePoint(i)}>×</button></div>{/each}
    <button disabled={points.length >= 16} onclick={() => { const last=points.at(-1)!; points.push({ celsius:last.celsius+5,value:last.value }); }}>{t("Добавить точку")}</button>
    <div class="row"><label>{t("Гистерезис,")} {displaySuffix}
      <TemperatureInput bind:value={hysteresis} min={0} max={10} difference/></label><label>Fallback PWM, %
      <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires explicit spinbutton for the native Value interface; verified by AT-SPI.) -->
      <input type="number" role="spinbutton" bind:value={fallback} min={minDuty} max="100" aria-valuemin={minDuty} aria-valuemax="100" aria-valuenow={Number.isFinite(fallback) ? fallback : undefined} aria-invalid={!Number.isInteger(fallback) || fallback < minDuty || fallback > 100}/></label></div>
  {/if}
  {#if pollError}<p role="alert">{t(pollError)}</p>{/if}
  {#if error}<p class="error" role="alert">{t(error)}</p>{/if}{#if notice}<p role="status">{t(notice)}</p>{/if}
  <button class="apply" disabled={busy || !device || !connected} onclick={apply}>{t("Применить охлаждение")}</button>
</section>
<style>
section{max-width:760px;display:grid;gap:18px}h2{font-size:16px}.row{display:flex;gap:14px;align-items:end}.summary{border:1px solid #35465b;background:#172334;border-radius:12px;padding:20px}.summary button{margin-top:18px}.presets{display:flex;gap:10px}p{line-height:1.6;color:#adbed2}.error{color:#ffb9c5}svg{width:100%;max-height:210px;background:#111c29;border-radius:10px;padding:15px}.apply{background:#326957;justify-self:start}strong{font-size:22px}.summary span{font-size:12px}input{max-width:300px}
</style>
