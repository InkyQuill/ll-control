<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { tick } from 'svelte';
  import { t } from './i18n.svelte';
  import { hex, rgb, type Layer, type Sensor } from './api';
  import type { FontChoice } from './font-upload';
  import DividerEditor from './DividerEditor.svelte';
  import TextStyleControls from './TextStyleControls.svelte';
  let {
    value = $bindable(),
    sensors,
    fontChoices,
  }: { value: Layer; sensors: Sensor[]; fontChoices: FontChoice[] } = $props();
  async function removeCycleSensor(
    target: Extract<Layer, { kind: 'sensor_cycle' }>,
    index: number,
    button: HTMLButtonElement,
  ) {
    if (target.sensors.length <= 1) return;
    const container = button.closest('article'),
      sources = target.sensors;
    sources.splice(index, 1);
    await tick();
    if (!container?.isConnected || value !== target || sources !== target.sensors) return;
    const inputs = container.querySelectorAll<HTMLSelectElement>('[data-cycle-source]');
    inputs[Math.min(index, inputs.length - 1)]?.focus();
  }
</script>

{#each [value] as layer (layer)}
  {#if layer.kind === 'divider'}<DividerEditor
      bind:layer={() => layer, (next) => (value = next)}
    />{/if}
  {#if layer.kind === 'meteor'}<StudioPopover label={t('Help')} icon="help"
      ><p>{t('MeteorShower settings are in the Meteors section.')}</p></StudioPopover
    >{/if}
  {#if layer.kind === 'weather'}<StudioPopover label={t('Help')} icon="help"
      ><p>{t('Rain/Snow settings are in the LCD animation section.')}</p></StudioPopover
    >{/if}
  {#if layer.kind === 'text'}<label
      >{t('Text')}<textarea bind:value={layer.text} maxlength="2048"></textarea></label
    >{/if}
  {#if layer.kind === 'sensor'}<label
      >{t('Sensor source')}<select
        aria-label={t('Sensor source')}
        value={layer.sensor_id}
        onchange={(event) => {
          const source = sensors.find((s) => s.id === event.currentTarget.value);
          if (source) {
            layer.sensor_id = source.id;
            layer.unit = source.unit;
          }
        }}
      >
        {#each sensors as source (source.id)}<option value={source.id}
            >{source.label} · {source.unit}</option
          >{/each}
        {#if !sensors.some((source) => source.id === layer.sensor_id)}<option
            value={layer.sensor_id}>{t('Unavailable:')} {layer.sensor_id}</option
          >{/if}
      </select></label
    ><label>{t('Label')}<input bind:value={layer.label} /></label><code>{layer.sensor_id}</code>
    <div class="row">
      <label
        >{t('Decimals')}<input type="number" bind:value={layer.decimals} min="0" max="2" /></label
      >{#if layer.unit === 'celsius'}<label
          >{t('Unit')}<select bind:value={layer.temperature_unit}
            ><option value="system">{t('Global settings')}</option><option value="celsius"
              >°C</option
            ><option value="fahrenheit">°F</option></select
          ></label
        >{/if}
    </div>{/if}
  {#if layer.kind === 'sensor_cycle'}
    <label
      >{t('Cycle interval, s')}<input
        type="number"
        min="1"
        max="3600"
        bind:value={layer.interval_seconds}
      /></label
    >
    <button
      disabled={layer.sensors.length >= 16}
      onclick={() => layer.sensors.push({ ...layer.sensors[layer.sensors.length - 1] })}
      >{t('Add a reading to the cycle')}</button
    >
    {#each layer.sensors as sensor, j (sensor)}
      <button
        disabled={layer.sensors.length === 1}
        onclick={(event) => removeCycleSensor(layer, j, event.currentTarget)}
        >{t('Remove reading')} {j + 1}</button
      >
      <label
        >{t('Source')}
        {j + 1}<select
          data-cycle-source
          value={sensor.sensor_id}
          onchange={(event) => {
            const selected = sensors.find((s) => s.id === event.currentTarget.value);
            if (selected) {
              sensor.sensor_id = selected.id;
              sensor.unit = selected.unit;
            }
          }}
        >
          {#each sensors as source (source.id)}<option value={source.id}
              >{source.label} · {source.unit}</option
            >{/each}
          {#if !sensors.some((s) => s.id === sensor.sensor_id)}<option value={sensor.sensor_id}
              >{t('Unavailable:')} {sensor.sensor_id}</option
            >{/if}
        </select></label
      >
      <label>{t('Label')} {j + 1}<input bind:value={sensor.label} /></label>
      <label
        >{t('Decimals')}
        {j + 1}<input type="number" min="0" max="2" bind:value={sensor.decimals} /></label
      >
      {#if sensor.unit === 'celsius'}<label
          >{t('Unit')}
          {j + 1}<select bind:value={sensor.temperature_unit}
            ><option value="system">{t('Global settings')}</option><option value="celsius"
              >°C</option
            ><option value="fahrenheit">°F</option></select
          ></label
        >{/if}
    {/each}
  {/if}
  {#if layer.kind === 'sensor' || layer.kind === 'sensor_cycle'}
    <label
      ><input
        type="checkbox"
        checked={!!layer.parts}
        onchange={(e) => {
          layer.parts = e.currentTarget.checked
            ? {
                label: {
                  ...$state.snapshot(layer.style),
                  y: Math.max(0, layer.style.y - 45),
                  size: 22,
                },
                unit: {
                  ...$state.snapshot(layer.style),
                  y: Math.min(479, layer.style.y + 65),
                  size: 22,
                },
              }
            : null;
        }}
      />{t('Separate label and unit styles')}</label
    >
    {#if layer.parts}
      {#each ['label', 'unit'] as part (part)}
        <fieldset>
          <legend
            >{part === 'label' ? '' + t('Sensor label') + '' : '' + t('Sensor unit') + ''}</legend
          >
          <TextStyleControls
            bind:style={layer.parts[part as 'label' | 'unit']}
            fonts={fontChoices}
          />
        </fieldset>
      {/each}
    {/if}
  {/if}
  {#if layer.kind === 'clock'}<label
      >{t('Clock content')}<select
        aria-label={t('Clock content')}
        value={layer.format ?? 'time'}
        onchange={(e) => {
          layer.format = e.currentTarget.value as typeof layer.format;
        }}
        ><option value="time">{t('Time')}</option><option value="time_without_period"
          >{t('Time without AM/PM')}</option
        ><option value="date">{t('Date YYYY/MM/DD')}</option><option value="period"
          >A.M. / P.M.</option
        ></select
      ></label
    >
    <div class="row">
      <label><input type="checkbox" bind:checked={layer.twenty_four_hour} />{t('24 hours')}</label
      ><label><input type="checkbox" bind:checked={layer.show_seconds} />{t('Seconds')}</label>
    </div>{/if}
  {#if layer.kind === 'image'}
    <label
      ><input
        type="checkbox"
        checked={!!layer.watermark_position}
        onchange={(e) =>
          (layer.watermark_position = e.currentTarget.checked
            ? { horizontal: 50, vertical: 50 }
            : null)}
      />{t('L-Connect watermark positioning')}</label
    >
    {#if layer.watermark_position}
      <StudioPopover label={t('Help')} icon="help"
        ><p>
          {t(
            'The image center is a percentage of the screen. A value of 0 positions it 22 px from the edge, matching L-Connect.',
          )}
        </p></StudioPopover
      >
      <label
        >{t('Watermark horizontal position')}<input
          type="number"
          min="0"
          max="100"
          bind:value={layer.watermark_position.horizontal}
        /></label
      >
      <label
        >{t('Watermark vertical position')}<input
          type="number"
          min="0"
          max="100"
          bind:value={layer.watermark_position.vertical}
        /></label
      >
    {/if}
    <label
      >{t('Scale')}<select bind:value={layer.fit}
        ><option value="crop">{t('Fill and crop')}</option><option value="contain"
          >{t('Fit')}</option
        ><option value="contain_native">{t('Fit without upscaling (watermark)')}</option><option
          value="stretch">{t('Stretch')}</option
        ></select
      ></label
    >
    <label
      ><input
        type="checkbox"
        checked={!!layer.shadow}
        onchange={(e) =>
          (layer.shadow = e.currentTarget.checked
            ? { x: 2, y: 2, blur: 2, color: [0, 0, 0], opacity: 60 }
            : null)}
      />{t('Image shadow')}</label
    >
    {#if layer.shadow}
      <div class="row">
        <label
          >{t('Image shadow offset X')}<input
            type="number"
            min="-32"
            max="32"
            bind:value={layer.shadow.x}
          /></label
        >
        <label
          >{t('Image shadow offset Y')}<input
            type="number"
            min="-32"
            max="32"
            bind:value={layer.shadow.y}
          /></label
        >
        <label
          >{t('Image shadow blur')}<input
            type="number"
            min="0"
            max="16"
            bind:value={layer.shadow.blur}
          /></label
        >
        <label
          >{t('Image shadow opacity, %')}<input
            type="number"
            min="0"
            max="100"
            bind:value={layer.shadow.opacity}
          /></label
        >
        <label
          >{t('Image shadow color')}<input
            type="color"
            value={hex(layer.shadow.color)}
            oninput={(e) => {
              if (layer.shadow) layer.shadow.color = rgb(e.currentTarget.value);
            }}
          /></label
        >
      </div>
    {/if}
    <label>{t('Opacity')}<input type="range" min="0" max="255" bind:value={layer.opacity} /></label>
    <div class="row">
      {#each ['x', 'y', 'width', 'height'] as key (key)}<label
          >{key}<input
            type="number"
            disabled={!!layer.watermark_position && (key === 'x' || key === 'y')}
            min="0"
            max="480"
            value={layer[key as 'x' | 'y' | 'width' | 'height']}
            oninput={(e) => {
              if (layer.kind === 'image')
                layer[key as 'x' | 'y' | 'width' | 'height'] = Number(e.currentTarget.value);
            }}
          /></label
        >{/each}
    </div>
  {:else if layer.kind !== 'divider' && layer.kind !== 'weather' && layer.kind !== 'meteor' && layer.kind !== 'snow3d' && layer.kind !== 'fireworks'}
    <TextStyleControls bind:style={layer.style} fonts={fontChoices} />
  {/if}
{/each}

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .row > label {
    flex: 1;
    min-width: 72px;
  }
  label {
    margin-bottom: 10px;
  }
  code {
    overflow-wrap: anywhere;
  }
  fieldset {
    margin: 12px 0;
    padding: 12px;
  }
</style>
