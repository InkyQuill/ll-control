<script lang="ts">
  import { SvelteMap } from 'svelte/reactivity';
  import { t } from './i18n.svelte';
  import { ValidationError } from './validation-error';
  import TemperatureInput from './TemperatureInput.svelte';
  import { presentation } from './presentation.svelte';
  import { temperatureSuffix, temperatureFromCelsius } from './temperature';
  const displayUnit = $derived(presentation.settings?.temperature_unit ?? 'celsius');
  const displaySuffix = $derived(temperatureSuffix(displayUnit));
  import { untrack } from 'svelte';
  import { initialWeatherCpuLoad } from './weather-settings';
  import { rpc, message, rgb, hex, type Sensor, type SensorDefaultsState } from './api';
  type Unit = 'celsius' | 'percent' | 'rpm';
  interface Color {
    r: number;
    g: number;
    b: number;
  }
  interface Config {
    brightness_percent?: number;
    direction?: string;
    sensor_id: string;
    unit: Unit;
    low_value: number;
    high_value: number;
    low_colors: Color[];
    high_colors: Color[];
    low_speed_percent: number | null;
    high_speed_percent: number | null;
  }
  interface ModeDefaults {
    color: Color;
    additional_colors?: Color[];
    speed_percent: number;
    brightness_percent: number;
    direction: string;
  }
  interface Profiles {
    modes: { mode: string; bindings: Config[] }[];
  }
  interface Dynamic {
    profiles?: Profiles;
    mode_defaults?: ModeDefaults | null;
    direction_supported?: boolean;
    base_mode: string | null;
    color_count: number | null;
    speed_supported: boolean;
    requested: Config | null;
    applied: { value: number; colors: Color[]; speed_percent: number | null } | null;
    degraded: boolean;
    suppressed_by_thermal: boolean;
    last_error: string | null;
  }
  let {
    device,
    connected,
    sensors,
    disabled = false,
    busy = $bindable(false),
    onpolicychange,
  }: {
    device: string;
    connected: boolean;
    sensors: Sensor[];
    disabled?: boolean;
    busy?: boolean;
    onpolicychange?: (dev: string, mode: string, enabled: boolean) => void;
  } = $props();
  let open = $state(false),
    current = $state<Dynamic | null>(null),
    error = $state<ValidationError | string | null>(null),
    notice = $state('');
  let loaded = $state(false),
    draftMode = $state(''),
    draftDevice = $state(''),
    sensorId = $state(''),
    unit = $state<Unit>('celsius');
  let low = $state<number | undefined>(30),
    high = $state<number | undefined>(60),
    lowSpeed = $state<number | undefined>(0),
    highSpeed = $state<number | undefined>(100);
  let colors = $state<{ low: string; high: string }[]>([]);
  let fixedBrightness = $state<number | undefined>(100),
    inheritBrightness = $state(true),
    fixedDirection = $state('inherit');
  interface Draft {
    sensorId: string;
    unit: Unit;
    low: number | undefined;
    high: number | undefined;
    lowSpeed: number | undefined;
    highSpeed: number | undefined;
    colors: { low: string; high: string }[];
    brightness: number | undefined;
    inheritBrightness: boolean;
    direction: string;
  }
  const drafts = new SvelteMap<string, Draft>();
  let draftOwner: { dev: string; mode: string; sensorId: string; unit: Unit } | null = null;
  let sourceRequest = 0,
    editRevision = 0,
    choosing = $state(false);
  const normalize = (mode: string) => (mode === 'static_color' ? 'static' : mode);
  const key = (dev: string, mode: string, id: string, u: Unit) =>
    JSON.stringify([dev, normalize(mode), id, u]);
  let revision = 0,
    generation = 0;
  let pollError = $state('');
  let eligible = $derived(
    sensors.filter((s) => s.unit === 'celsius' || s.unit === 'percent' || s.unit === 'rpm'),
  );
  let thresholdMax = $derived(unit === 'rpm' ? 3600 : 100);
  const invalidSpeed = (value: number | undefined) =>
    value === undefined || !Number.isInteger(value) || value < 0 || value > 100;
  let unavailable = $derived(busy || disabled || !connected || !device);
  let modeChanged = $derived(
    loaded && (draftDevice !== device || draftMode !== current?.base_mode),
  );
  $effect(() => {
    const dev = device,
      online = connected,
      visible = open,
      parentBusy = disabled;
    untrack(cacheDraft);
    sourceRequest++;
    choosing = false;
    generation++;
    revision++;
    current = null;
    pollError = '';
    error = null;
    notice = '';
    if (!dev || !online || !visible || parentBusy) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      const started = revision;
      try {
        if (!busy) {
          const result = await rpc<Dynamic>('get_dynamic_rgb', { dev });
          if (!cancelled && started === revision) {
            current = result;
            pollError = '';
          }
        }
      } catch (e) {
        if (!cancelled && started === revision) {
          current = null;
          pollError = message(e);
        }
      }
      if (!cancelled) timer = setTimeout(poll, 1000);
    }
    timer = setTimeout(poll, 0);
    return () => {
      generation++;
      cancelled = true;
      clearTimeout(timer);
    };
  });
  function cacheDraft() {
    if (!draftOwner?.sensorId) return;
    const o = draftOwner;
    drafts.set(key(o.dev, o.mode, o.sensorId, o.unit), {
      sensorId: o.sensorId,
      unit: o.unit,
      low,
      high,
      lowSpeed,
      highSpeed,
      colors: colors.map((c) => ({ ...c })),
      brightness: fixedBrightness,
      inheritBrightness,
      direction: fixedDirection,
    });
  }
  function markEdit() {
    editRevision++;
    sourceRequest++;
    choosing = false;
  }
  function defaults(id: string, u: Unit, saved?: Config): Draft {
    const metadata = current?.profiles ? current.mode_defaults : null;
    const palette = metadata ? [metadata.color, ...(metadata.additional_colors ?? [])] : null;
    return {
      sensorId: id,
      unit: u,
      low: saved?.low_value ?? (u === 'rpm' ? 800 : 30),
      high: saved?.high_value ?? (u === 'rpm' ? 1200 : 60),
      lowSpeed: saved?.low_speed_percent ?? metadata?.speed_percent ?? 0,
      highSpeed: saved?.high_speed_percent ?? metadata?.speed_percent ?? 100,
      colors: Array.from({ length: current?.color_count ?? 0 }, (_, i) => ({
        low: saved?.low_colors[i]
          ? colorHex(saved.low_colors[i])
          : palette?.[i]
            ? colorHex(palette[i])
            : '#0000ff',
        high: saved?.high_colors[i]
          ? colorHex(saved.high_colors[i])
          : palette?.[i]
            ? colorHex(palette[i])
            : '#ff0000',
      })),
      brightness: saved?.brightness_percent ?? metadata?.brightness_percent ?? 100,
      inheritBrightness: saved ? saved.brightness_percent === undefined : !metadata,
      direction: saved?.direction ?? (metadata && !saved ? 'forward' : 'inherit'),
    };
  }
  function useDraft(draft: Draft) {
    sensorId = draft.sensorId;
    unit = draft.unit;
    low = draft.low;
    high = draft.high;
    lowSpeed = draft.lowSpeed;
    highSpeed = draft.highSpeed;
    colors = draft.colors.map((c) => ({ ...c }));
    fixedBrightness = draft.brightness;
    inheritBrightness = draft.inheritBrightness;
    fixedDirection = draft.direction;
    draftOwner = { dev: device, mode: draftMode, sensorId, unit };
  }
  function savedBinding(id: string, u: Unit): Config | undefined {
    return current?.profiles?.modes
      .find((p) => normalize(p.mode) === normalize(draftMode))
      ?.bindings.find((c) => c.sensor_id === id && c.unit === u);
  }
  function chooseSensor() {
    const id = sensorId;
    cacheDraft();
    sourceRequest++;
    choosing = false;
    const row = eligible.find((s) => s.id === id),
      u = (row?.unit as Unit | undefined) ?? unit;
    useDraft(drafts.get(key(device, draftMode, id, u)) ?? defaults(id, u, savedBinding(id, u)));
  }
  async function load() {
    if (!current?.base_mode || current.color_count === null) return;
    cacheDraft();
    choosing = false;
    const epoch = generation,
      request = ++sourceRequest,
      edit = editRevision,
      sourceMode = current.base_mode,
      dev = device;
    const saved =
      current.requested ??
      current.profiles?.modes
        .find((p) => normalize(p.mode) === normalize(current!.base_mode!))
        ?.bindings.at(-1);
    draftDevice = device;
    draftMode = current.base_mode;
    loaded = true;
    error = null;
    notice = 'Настройки загружены в редактор.';
    useDraft(defaults(saved?.sensor_id ?? '', saved?.unit ?? 'celsius', saved));
    if (saved || !current.profiles) return;
    choosing = true;
    try {
      const state = await rpc<SensorDefaultsState>('get_sensor_defaults');
      if (
        epoch !== generation ||
        request !== sourceRequest ||
        edit !== editRevision ||
        device !== dev ||
        current?.base_mode !== sourceMode
      )
        return;
      const binding = initialWeatherCpuLoad(state);
      if (binding?.unit === 'percent') useDraft(defaults(binding.sensor_id, binding.unit));
    } catch {
      /* Keep an explicit chooser if semantic CPU Load is not available. */
    } finally {
      if (epoch === generation && request === sourceRequest) choosing = false;
    }
  }
  function colorHex(value: Color): string {
    return hex([value.r, value.g, value.b]);
  }
  function color(value: string): Color {
    const [r, g, b] = rgb(value);
    return { r, g, b };
  }
  async function apply(off = false) {
    if (unavailable) return;
    const dev = device,
      started = generation;
    revision++;
    busy = true;
    error = null;
    notice = '';
    try {
      let config: Config | null = null;
      if (!off) {
        if (!sensorId || !loaded || modeChanged || current?.color_count !== colors.length)
          throw new ValidationError('Загрузите текущий режим и выберите датчик');
        const max = unit === 'rpm' ? 3600 : 100;
        if (
          low === undefined ||
          high === undefined ||
          !Number.isInteger(low) ||
          !Number.isInteger(high) ||
          low < 0 ||
          high < low ||
          high > max
        )
          throw new ValidationError(
            unit === 'celsius'
              ? 'Пороги температуры должны соответствовать целым °C от 0 до {0}; нижний не выше верхнего.'
              : 'Пороги должны быть целыми числами от 0 до {0}, нижний не выше верхнего',
            [max],
          );
        if (current.speed_supported && [lowSpeed, highSpeed].some(invalidSpeed))
          throw new ValidationError('Скорость должна быть целым числом от 0 до 100');
        config = {
          sensor_id: sensorId,
          unit,
          low_value: low,
          high_value: high,
          low_colors: colors.map((c) => color(c.low)),
          high_colors: colors.map((c) => color(c.high)),
          low_speed_percent: current.speed_supported ? lowSpeed! : null,
          high_speed_percent: current.speed_supported ? highSpeed! : null,
        };
        if (current.profiles) {
          if (!inheritBrightness) {
            if (invalidSpeed(fixedBrightness))
              throw new ValidationError('Яркость должна быть целым числом от0 до100');
            config.brightness_percent = fixedBrightness!;
          }
          if (fixedDirection !== 'inherit')
            config.direction = current.direction_supported ? fixedDirection : 'forward';
        }
        cacheDraft();
      }
      const result = await rpc<Dynamic>('set_dynamic_rgb', { dev, config });
      if (started !== generation) return;
      sourceRequest++;
      current = result;
      if (result.base_mode) onpolicychange?.(dev, result.base_mode, !!result.requested);
      if (result.last_error) throw new Error(result.last_error);
      notice = off
        ? 'Режим по датчику отключён.'
        : 'Режим по датчику сохранён. Фактическое состояние показано выше.';
    } catch (e) {
      if (started === generation) error = e instanceof ValidationError ? e : message(e);
    } finally {
      revision++;
      busy = false;
    }
  }
</script>

<details ontoggle={(event) => (open = event.currentTarget.open)}>
  <summary>{t('Подсветка помпы по датчику')}</summary>
  <p>
    {t(
      'Выберите и примените обычный режим помпы выше. Здесь настраиваются цвета, скорость и фиксированные параметры выбранного источника. Настройки других источников и режимов сохраняются. Ручная настройка помпы отменяет этот режим.',
    )}
  </p>
  {#if current}
    <p>
      {t('Режим помпы:')}
      {current.base_mode ?? t('не задан')}
      {t('· По датчику:')}
      {current.requested ? '' + t('включён') + '' : '' + t('выключен') + ''}
    </p>
    {#if current.applied}<p>
        {t('Применённое значение:')}
        {current.requested?.unit === 'celsius'
          ? `${temperatureFromCelsius(current.applied.value, displayUnit).toFixed(1)} ${displaySuffix}`
          : current.applied.value}
      </p>{/if}
    {#if current.degraded}<p role="status">
        {current.suppressed_by_thermal
          ? '' + t('Датчик недоступен; режим по датчику ожидает восстановления.') + ''
          : '' + t('Датчик недоступен — используется обычная подсветка.') + ''}
      </p>{/if}
    {#if current.suppressed_by_thermal}<p role="status">
        {t('Подсветка временно перекрыта температурным оповещением.')}
      </p>{/if}
    {#if current.last_error}<p role="alert"><span>{current.last_error}</span></p>{/if}
  {/if}
  <button
    disabled={unavailable || !current?.base_mode || current.color_count === null}
    onclick={load}>{t('Загрузить настройки режима по датчику')}</button
  >
  {#if loaded}
    {#if modeChanged}<p role="alert">
        {t('Устройство или режим помпы изменились. Загрузите настройки заново перед применением.')}
      </p>{/if}
    <div oninput={markEdit} onchange={markEdit}>
      {#if choosing}<p role="status">{t('Чтение источника CPU Load…')}</p>{/if}
      <label
        >{t('Датчик подсветки')}<select
          bind:value={sensorId}
          disabled={unavailable || modeChanged}
          onchange={chooseSensor}
          ><option value="">{t('Выберите источник')}</option
          >{#each eligible as sensor (sensor.id)}<option value={sensor.id}
              >{sensor.label} · {sensor.unit} · {sensor.status}</option
            >{/each}{#if sensorId && !eligible.some((s) => s.id === sensorId)}<option
              value={sensorId}>{t('Недоступен:')} {sensorId}</option
            >{/if}</select
        ></label
      >
      <div class="row">
        <label
          >{t('Нижний порог (')}{unit === 'celsius' ? displaySuffix : unit})
          {#if unit === 'celsius'}<TemperatureInput
              describedby={displayUnit === 'fahrenheit'
                ? 'dynamic-rgb-temperature-grid'
                : undefined}
              bind:value={low}
              min={0}
              max={thresholdMax}
              step={1}
              invalid={!Number.isInteger(low) || (low ?? Infinity) > (high ?? -1)}
            />{:else}
            <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires the native Value interface.) -->
            <input
              type="number"
              role="spinbutton"
              min="0"
              max={thresholdMax}
              aria-valuemin="0"
              aria-valuemax={thresholdMax}
              aria-valuenow={Number.isFinite(low) ? low : undefined}
              aria-invalid={!Number.isInteger(low) ||
                (low ?? -1) < 0 ||
                (low ?? Infinity) > thresholdMax ||
                (low ?? Infinity) > (high ?? -1)}
              bind:value={low}
            />{/if}</label
        ><label
          >{t('Верхний порог (')}{unit === 'celsius' ? displaySuffix : unit})
          {#if unit === 'celsius'}<TemperatureInput
              describedby={displayUnit === 'fahrenheit'
                ? 'dynamic-rgb-temperature-grid'
                : undefined}
              bind:value={high}
              min={0}
              max={thresholdMax}
              step={1}
              invalid={!Number.isInteger(high) || (high ?? -1) < (low ?? Infinity)}
            />{:else}
            <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires the native Value interface.) -->
            <input
              type="number"
              role="spinbutton"
              min="0"
              max={thresholdMax}
              aria-valuemin="0"
              aria-valuemax={thresholdMax}
              aria-valuenow={Number.isFinite(high) ? high : undefined}
              aria-invalid={!Number.isInteger(high) ||
                (high ?? -1) < 0 ||
                (high ?? Infinity) > thresholdMax ||
                (high ?? -1) < (low ?? Infinity)}
              bind:value={high}
            />{/if}</label
        >
      </div>
      {#if unit === 'celsius' && displayUnit === 'fahrenheit'}<p id="dynamic-rgb-temperature-grid">
          {t(
            'Пороги этого режима хранятся целыми °C. В °F используйте шаг 1,8° (например, 86 → 87,8°F).',
          )}
        </p>{/if}
      {#each colors as pair, i (i)}<div class="row">
          <label>{t('Нижний цвет')} {i + 1}<input type="color" bind:value={pair.low} /></label
          ><label>{t('Верхний цвет')} {i + 1}<input type="color" bind:value={pair.high} /></label>
        </div>{/each}
      {#if current?.speed_supported}<div class="row">
          <label
            >{t('Нижняя скорость, %')}
            <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires explicit spinbutton for the native Value interface.) -->
            <input
              type="number"
              role="spinbutton"
              min="0"
              max="100"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Number.isFinite(lowSpeed) ? lowSpeed : undefined}
              aria-invalid={invalidSpeed(lowSpeed)}
              bind:value={lowSpeed}
            /></label
          ><label
            >{t('Верхняя скорость, %')}
            <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires explicit spinbutton for the native Value interface.) -->
            <input
              type="number"
              role="spinbutton"
              min="0"
              max="100"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Number.isFinite(highSpeed) ? highSpeed : undefined}
              aria-invalid={invalidSpeed(highSpeed)}
              bind:value={highSpeed}
            /></label
          >
        </div>{/if}
      {#if current?.profiles}
        <label
          ><input type="checkbox" bind:checked={inheritBrightness} />{t(
            'Яркость обычного режима',
          )}</label
        >
        {#if !inheritBrightness}<label
            >{t('Яркость по датчику, %')}
            <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK needs explicit spinbutton for native Value.) -->
            <input
              type="number"
              role="spinbutton"
              min="0"
              max="100"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Number.isFinite(fixedBrightness) ? fixedBrightness : undefined}
              aria-invalid={invalidSpeed(fixedBrightness)}
              bind:value={fixedBrightness}
            /></label
          >{/if}
        {#if current.direction_supported}<label
            >{t('Направление по датчику')}<select bind:value={fixedDirection}
              ><option value="inherit">{t('Как в обычном режиме')}</option><option value="forward"
                >{t('Вперёд')}</option
              ><option value="reverse">{t('Назад')}</option></select
            ></label
          >{/if}
      {/if}
    </div>
    <button disabled={unavailable || modeChanged || !sensorId || !current} onclick={() => apply()}
      >{t('Применить режим по датчику')}</button
    >
  {/if}
  <button disabled={unavailable || !current?.requested} onclick={() => apply(true)}
    >{t('Отключить режим по датчику')}</button
  >
  {#if pollError}<p role="alert">
      {t('Не удалось прочитать режим подсветки по датчику:')} <span>{pollError}</span>
    </p>{/if}
  {#if error}<p role="alert">
      {#if error instanceof ValidationError}{t(error.resource, ...error.values)}{:else}{t(
          'Ошибка подсветки по датчику:',
        )} <span>{error}</span>{/if}
    </p>{/if}{#if notice}<p role="status">{t(notice)}</p>{/if}
</details>

<style>
  details {
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 16px;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  label {
    display: grid;
    gap: 6px;
    margin: 12px 0;
  }
  .row {
    display: flex;
    gap: 15px;
    flex-wrap: wrap;
  }
  button {
    margin: 8px 8px 0 0;
  }
  p {
    line-height: 1.6;
  }
  [role='alert'] {
    color: var(--danger);
  }
</style>
