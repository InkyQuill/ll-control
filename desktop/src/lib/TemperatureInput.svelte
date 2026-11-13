<script lang="ts">
  import { presentation } from './presentation.svelte';
  import { temperatureFromCelsius, temperatureToCelsius } from './temperature';
  let {
    value = $bindable(),
    min,
    max,
    step = 'any',
    difference = false,
    disabled = false,
    invalid = false,
    curve = false,
    describedby,
  }: {
    value: number | undefined;
    min: number;
    max: number;
    step?: number | 'any';
    difference?: boolean;
    disabled?: boolean;
    invalid?: boolean;
    curve?: boolean;
    describedby?: string;
  } = $props();
  const unit = $derived(presentation.settings?.temperature_unit ?? 'celsius');
  const displayMin = $derived(temperatureFromCelsius(min, unit, difference));
  const displayMax = $derived(temperatureFromCelsius(max, unit, difference));
  const displayedValue = $derived(
    value !== undefined && Number.isFinite(value)
      ? Number(temperatureFromCelsius(value, unit, difference).toFixed(8))
      : undefined,
  );
</script>

<!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK native Value interface requires an explicit spinbutton role.) -->
<input
  type="number"
  role="spinbutton"
  data-curve-temperature={curve ? '' : undefined}
  {disabled}
  min={displayMin}
  max={displayMax}
  step={step === 'any' ? 'any' : temperatureFromCelsius(step, unit, true)}
  aria-describedby={describedby}
  aria-valuemin={displayMin}
  aria-valuemax={displayMax}
  aria-valuenow={displayedValue}
  aria-invalid={invalid ||
    value === undefined ||
    !Number.isFinite(value) ||
    value < min ||
    value > max}
  bind:value={
    () => displayedValue, (displayed) => (value = temperatureToCelsius(displayed, unit, difference))
  }
/>
