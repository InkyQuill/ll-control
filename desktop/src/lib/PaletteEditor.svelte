<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import ExperimentalNotice from './ExperimentalNotice.svelte';
  import { t } from './i18n.svelte';
  import { onDestroy, untrack } from 'svelte';
  import {
    rpc,
    message,
    hex,
    rgb,
    type Theme,
    type Sensor,
    type MeteorSensor,
    type FireworksSensor,
    type EffectSensor,
    type SensorDefaultsState,
    fireworksColors,
  } from './api';
  import {
    weatherRangeValid,
    weatherRangeMaximum,
    initialWeatherCpuLoad,
  } from './weather-settings';
  let {
    theme = $bindable(),
    sensors,
    disabled = false,
  }: { theme: Theme; sensors: Sensor[]; disabled?: boolean } = $props();
  const active = $derived(
    theme.layers.find((layer) => layer.kind === 'meteor' || layer.kind === 'fireworks'),
  );
  const family = $derived(active?.kind === 'fireworks' ? 'fireworks' : 'meteors');
  const indices = $derived(
    Array.from({ length: active?.effect.colors.length ?? 0 }, (_, index) => index),
  );
  const source = $derived(active?.effect.sensor);
  const binding = $derived(source?.binding);
  const sources = $derived(
    sensors.filter((sensor) => ['celsius', 'percent', 'rpm'].includes(sensor.unit)),
  );
  const valid = $derived(weatherRangeValid(binding));
  const maximum = $derived(weatherRangeMaximum(binding));
  const unavailable = $derived(
    binding &&
      !sources.some(
        (sensor) =>
          sensor.id === binding.sensor_id &&
          sensor.unit === binding.unit &&
          sensor.status === 'valid' &&
          sensor.value !== null &&
          sensor.age_ms !== null &&
          sensor.age_ms <= 3000,
      ),
  );
  let request = 0,
    choosing = $state(false),
    sourceError = $state('');
  $effect(() => {
    void theme;
    untrack(() => {
      request++;
      choosing = false;
      sourceError = '';
    });
  });
  onDestroy(() => {
    request++;
  });
  function key(sensor: { id: string; unit: Sensor['unit'] }) {
    return JSON.stringify([sensor.id, sensor.unit]);
  }
  function savedKey(source: { binding: EffectSensor }) {
    return key({ id: source.binding.sensor_id, unit: source.binding.unit });
  }
  function canChoose(sensor: Sensor) {
    if (!active || !source || savedKey(source) === key(sensor)) return true;
    return (
      valid &&
      ((active.effect.retained_sensors ?? []).some((source) => savedKey(source) === key(sensor)) ||
        (active.effect.retained_sensors ?? []).length < 63)
    );
  }
  function createMeteorSource(binding: EffectSensor): MeteorSensor {
    return {
      binding,
      low_colors: [
        [0, 255, 0],
        [0, 0, 255],
      ],
      high_colors: [
        [0, 255, 0],
        [0, 0, 255],
      ],
    };
  }
  function createFireworksSource(binding: EffectSensor): FireworksSensor {
    return { binding, low_colors: fireworksColors(), high_colors: fireworksColors() };
  }
  type ColourSource = { binding: EffectSensor };
  function replaceSource<T extends ColourSource>(
    effect: { sensor: T | null; retained_sensors?: T[] },
    identity: string,
    fresh: () => T,
  ) {
    const previous = effect.sensor,
      retained = effect.retained_sensors ?? [],
      saved = retained.find((source) => savedKey(source) === identity);
    effect.retained_sensors = retained.filter((source) => savedKey(source) !== identity);
    if (previous) effect.retained_sensors.push(previous);
    effect.sensor = saved ?? fresh();
  }
  function choose(value: string) {
    const sensor = sources.find((sensor) => key(sensor) === value);
    if (!active || !sensor || !canChoose(sensor)) return false;
    if (source && savedKey(source) === value) return true;
    request++;
    choosing = false;
    sourceError = '';
    const binding = {
      sensor_id: sensor.id,
      unit: sensor.unit,
      low: sensor.unit === 'rpm' ? 800 : 30,
      high: sensor.unit === 'rpm' ? 1200 : 60,
    };
    if (active.kind === 'meteor')
      replaceSource(active.effect, value, () => createMeteorSource(binding));
    else replaceSource(active.effect, value, () => createFireworksSource(binding));
    return true;
  }
  async function dynamic(enabled: boolean) {
    if (!active) return;
    const currentRequest = ++request,
      owner = theme,
      captured = active,
      effect = active.effect;
    effect.dynamic = enabled;
    choosing = false;
    sourceError = '';
    if (!enabled || effect.sensor) return;
    choosing = true;
    const current = () =>
      currentRequest === request &&
      theme === owner &&
      active?.effect === effect &&
      effect.dynamic &&
      !effect.sensor;
    try {
      const state = await rpc<SensorDefaultsState>('get_sensor_defaults');
      if (!current()) return;
      const proposed = initialWeatherCpuLoad(state);
      if (proposed) {
        const identity = JSON.stringify([proposed.sensor_id, proposed.unit]);
        if (captured.kind === 'meteor')
          replaceSource(captured.effect, identity, () => createMeteorSource(proposed));
        else replaceSource(captured.effect, identity, () => createFireworksSource(proposed));
      }
    } catch (error) {
      if (current()) sourceError = message(error);
    } finally {
      if (currentRequest === request) choosing = false;
    }
  }
  function color(value: string): [number, number, number] {
    const c = rgb(value);
    return [c[0], c[1], c[2]];
  }
</script>

{#if active}
  <fieldset {disabled}>
    <legend>{active.kind === 'fireworks' ? '' + t('Firework') + '' : '' + t('Meteors') + ''}</legend
    >
    <ExperimentalNotice
      feature="procedural"
      message="Procedural effects are implemented, but their appearance, timing, and failure behavior on LCDV2 are not fully verified. Use at your own risk."
    />
    <StudioPopover label={t('Help')} icon="help"
      ><p>{t('Text, clocks and images remain above the animation.')}</p></StudioPopover
    >
    <fieldset disabled={active.effect.dynamic}>
      <legend>{t('Static settings')} {t(family)}</legend>
      {#if active.kind === 'meteor'}<label
          >{t('Meteor speed')}<select bind:value={active.effect.speed}
            ><option value={1}>1</option><option value={2}>2</option><option value={3}>3</option
            ></select
          ></label
        >{:else}<label
          >{t('Fireworks intensity')}<select bind:value={active.effect.intensity}
            ><option value={1}>1</option><option value={2}>2</option><option value={3}>3</option
            ></select
          ></label
        >{/if}
      {#each indices as index (index)}
        {@const colours = active.effect.colors}
        <label
          >{t('Color')}
          {t(family)}
          {index + 1}<input
            type="color"
            value={hex(colours[index])}
            oninput={(event) => {
              if (active?.effect.colors === colours)
                colours[index] = color(event.currentTarget.value);
            }}
          /></label
        >
      {/each}
    </fieldset>
    <label
      ><input
        type="checkbox"
        checked={active.effect.dynamic}
        onchange={(event) => dynamic(event.currentTarget.checked)}
      />{t('Control effect with a sensor')}</label
    >
    {#if source || active.effect.dynamic}
      <fieldset disabled={!active.effect.dynamic}>
        <legend>{t('Sensor')} {t(family)}</legend>
        <label
          >{t('Effect source')}<select
            value={source ? savedKey(source) : ''}
            onchange={(event) => {
              if (!choose(event.currentTarget.value))
                event.currentTarget.value = source ? savedKey(source) : '';
            }}
          >
            {#if !source}<option value="" disabled>{t('Select a sensor')}</option>{/if}
            {#each sources as sensor (key(sensor))}<option
                value={key(sensor)}
                disabled={!canChoose(sensor)}>{sensor.label} · {sensor.unit}</option
              >{/each}
            {#if source && !sources.some((sensor) => key(sensor) === savedKey(source))}<option
                value={savedKey(source)}>{t('Unavailable:')} {source.binding.sensor_id}</option
              >{/if}
          </select></label
        >
        {#if source}
          <StudioPopover label={t('Help')} icon="help"
            ><p>
              {t('Threshold source units:')}
              {source.binding.unit === 'celsius'
                ? '°C'
                : source.binding.unit === 'percent'
                  ? '%'
                  : 'RPM'}{t('. Range: 0–')}{maximum}.
            </p></StudioPopover
          >
          <label
            >{t('Lower effect threshold')}<input
              type="number"
              min="0"
              max={maximum}
              step="1"
              bind:value={source.binding.low}
            /></label
          >
          <label
            >{t('Upper effect threshold')}<input
              type="number"
              min="0"
              max={maximum}
              step="1"
              bind:value={source.binding.high}
            /></label
          >
          {#each indices as index (index)}
            {@const paletteSource = source}
            <label
              >{t('Lower color')}
              {t(family)}
              {index + 1}<input
                type="color"
                value={hex(paletteSource.low_colors[index])}
                oninput={(event) => {
                  if (source === paletteSource)
                    paletteSource.low_colors[index] = color(event.currentTarget.value);
                }}
              /></label
            >
            <label
              >{t('Upper color')}
              {t(family)}
              {index + 1}<input
                type="color"
                value={hex(paletteSource.high_colors[index])}
                oninput={(event) => {
                  if (source === paletteSource)
                    paletteSource.high_colors[index] = color(event.currentTarget.value);
                }}
              /></label
            >
          {/each}
          {#if source.binding.low === source.binding.high}<StudioPopover
              label={t('Help')}
              icon="help"
              ><p>
                {t(
                  'At equal thresholds, the lower intensity and palette apply at the threshold; the upper settings apply above it.',
                )}
              </p></StudioPopover
            >{/if}
        {/if}
      </fieldset>
    {/if}
    {#if active.effect.dynamic && !source}<p role="status">
        {choosing
          ? '' + t('Reading the CPU Load source…') + ''
          : sourceError
            ? t('Could not read the CPU Load source: {0}. Select a sensor manually.', sourceError)
            : '' + t('Select an effect source: CPU Load is unavailable or ambiguous.') + ''}
      </p>{/if}
    {#if active.effect.dynamic && unavailable}<p role="status">
        {t('The source is unavailable: saved static settings and colors are used.')}
      </p>{/if}
    {#if !valid}<p role="status">
        {t('Enter integer thresholds; the lower threshold must not exceed the upper.')}
      </p>{/if}
    {#if (active.effect.retained_sensors ?? []).length >= 63}<p role="status">
        {t('64 sources are saved: you can select previously configured sources.')}
      </p>{/if}
  </fieldset>
{/if}

<style>
  fieldset {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
    margin: 16px 0;
  }
  label {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 10px 0;
    flex-wrap: wrap;
  }
  select,
  input {
    max-width: 100%;
  }
</style>
