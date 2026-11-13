<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import ExperimentalNotice from './ExperimentalNotice.svelte';
  import { t } from './i18n.svelte';
  import TemperatureInput from './TemperatureInput.svelte';
  import { presentation } from './presentation.svelte';
  import { temperatureSuffix, temperatureFromCelsius } from './temperature';
  const displayUnit = $derived(presentation.settings?.temperature_unit ?? 'celsius');
  const displaySuffix = $derived(temperatureSuffix(displayUnit));
  import { switchLcdPage, lcdPageKind, isProcedural } from './lcd-pages';
  import { onDestroy } from 'svelte';
  import {
    weatherRangeValid,
    weatherRangeMaximum,
    initialWeatherCpuLoad,
  } from './weather-settings';
  import {
    rpc,
    message,
    type Theme,
    type Sensor,
    type EffectSensor,
    type SensorDefaultsState,
  } from './api';
  let {
    theme = $bindable(),
    sensors,
    disabled = false,
  }: { theme: Theme; sensors: Sensor[]; disabled?: boolean } = $props();
  let choosingSource = $state(false),
    sourceError = $state('');
  let sourceRequest = 0;
  onDestroy(() => {
    sourceRequest++;
  });
  const weather = $derived(theme.layers.find((layer) => layer.kind === 'weather'));
  const sources = $derived(
    sensors.filter((source) => ['celsius', 'percent', 'rpm'].includes(source.unit)),
  );
  const binding = $derived(weather?.effect.sensor);
  const unavailable = $derived(
    binding &&
      !sources.some(
        (source) =>
          source.id === binding.sensor_id &&
          source.unit === binding.unit &&
          source.status === 'valid' &&
          source.value !== null &&
          source.age_ms !== null &&
          source.age_ms <= 3000,
      ),
  );
  const rangeMax = $derived(weatherRangeMaximum(binding));
  const thresholdsValid = $derived(weatherRangeValid(binding));
  const snow3d = $derived(theme.layers.find((layer) => layer.kind === 'snow3d'));
  const fireworks = $derived(theme.layers.find((layer) => layer.kind === 'fireworks'));
  const meteor = $derived(theme.layers.find((layer) => layer.kind === 'meteor'));
  const pageKind = $derived(lcdPageKind(theme));
  const settingsValid = $derived(
    (!fireworks ||
      (Number.isInteger(fireworks.effect.intensity) &&
        fireworks.effect.intensity >= 1 &&
        fireworks.effect.intensity <= 3 &&
        weatherRangeValid(fireworks.effect.sensor?.binding) &&
        (!fireworks.effect.dynamic || !!fireworks.effect.sensor))) &&
      (!snow3d ||
        (Number.isInteger(snow3d.effect.intensity) &&
          snow3d.effect.intensity >= 1 &&
          snow3d.effect.intensity <= 3)) &&
      thresholdsValid &&
      (!weather?.effect.dynamic || !!binding) &&
      (!meteor ||
        (Number.isInteger(meteor.effect.speed) &&
          meteor.effect.speed >= 1 &&
          meteor.effect.speed <= 3 &&
          weatherRangeValid(meteor.effect.sensor?.binding) &&
          (!meteor.effect.dynamic || !!meteor.effect.sensor))),
  );
  function canSelect(target: import('./api').LcdPageKind) {
    return (
      target === pageKind ||
      (settingsValid &&
        (isProcedural(theme.layers[0]) ||
          theme.layers.length < 32 ||
          !!theme.page_cache?.some((page) => page.page === target)))
    );
  }
  function select(value: string) {
    if (
      value !== 'none' &&
      value !== 'rain' &&
      value !== 'snow' &&
      value !== 'meteor' &&
      value !== 'snow3d' &&
      value !== 'fireworks'
    )
      return;
    if (!settingsValid) return false;
    sourceRequest++;
    choosingSource = false;
    sourceError = '';
    theme = switchLcdPage($state.snapshot(theme), value === 'none' ? 'basic' : value);
  }
  // Unit belongs to identity: a provider reusing an ID with a different unit
  // must not inherit thresholds in the previous scale.
  function sourceKey(source: { id: string; unit: Sensor['unit'] }) {
    return JSON.stringify([source.id, source.unit]);
  }
  function bindingKey(source: EffectSensor) {
    return sourceKey({ id: source.sensor_id, unit: source.unit });
  }
  function canChoose(source: Sensor) {
    if (!weather || !binding || bindingKey(binding) === sourceKey(source)) return true;
    if (!thresholdsValid) return false;
    return (
      (weather.effect.retained_sensors ?? []).some(
        (saved) => bindingKey(saved) === sourceKey(source),
      ) || (weather.effect.retained_sensors ?? []).length < 63
    );
  }
  function chooseSensor(key: string) {
    const source = sources.find((source) => sourceKey(source) === key);
    if (!source || !weather || !canChoose(source)) return false;
    const previous = weather.effect.sensor;
    if (previous && bindingKey(previous) === key) return true;
    sourceRequest++;
    choosingSource = false;
    sourceError = '';
    const retained = weather.effect.retained_sensors ?? [];
    const saved = retained.find((range) => bindingKey(range) === key);
    weather.effect.retained_sensors = retained.filter((range) => bindingKey(range) !== key);
    if (previous) weather.effect.retained_sensors.push({ ...previous });
    weather.effect.sensor = saved
      ? { ...saved }
      : {
          sensor_id: source.id,
          unit: source.unit,
          low: source.unit === 'rpm' ? 800 : 30,
          high: source.unit === 'rpm' ? 1200 : 60,
        };
    return true;
  }
  async function dynamic(enabled: boolean) {
    if (!weather) return;
    const request = ++sourceRequest,
      owner = theme,
      effect = weather.effect;
    choosingSource = false;
    sourceError = '';
    effect.dynamic = enabled;
    if (!enabled || effect.sensor) return;
    choosingSource = true;
    const current = () =>
      request === sourceRequest &&
      theme === owner &&
      weather?.effect === effect &&
      effect.dynamic &&
      !effect.sensor;
    try {
      const state = await rpc<SensorDefaultsState>('get_sensor_defaults');
      if (!current()) return;
      const proposed = initialWeatherCpuLoad(state);
      if (proposed) {
        const retained = effect.retained_sensors ?? [];
        effect.sensor =
          retained.find((saved) => bindingKey(saved) === bindingKey(proposed)) ?? proposed;
        effect.retained_sensors = retained.filter(
          (saved) => bindingKey(saved) !== bindingKey(proposed),
        );
      }
    } catch (error) {
      if (current()) sourceError = message(error);
    } finally {
      if (request === sourceRequest) choosingSource = false;
    }
  }
</script>

<fieldset {disabled}>
  <legend>{t('Анимация LCD')}</legend>
  <ExperimentalNotice
    feature="procedural"
    message="Процедурные эффекты реализованы, но их внешний вид, тайминг и отказы на LCDV2 проверены не полностью. Используйте на свой риск."
  />
  <label
    >{t('Эффект LCD')}<select
      value={pageKind === 'basic' ? 'none' : pageKind}
      onchange={(event) => {
        if (select(event.currentTarget.value) === false)
          event.currentTarget.value = pageKind === 'basic' ? 'none' : pageKind;
      }}
    >
      <option value="none" disabled={!settingsValid}>{t('Без эффекта')}</option>
      <option value="rain" disabled={!canSelect('rain')}>{t('Rain — дождь')}</option>
      <option value="snow" disabled={!canSelect('snow')}>{t('Snow — снег')}</option>
      <option value="snow3d" disabled={!canSelect('snow3d')}>{t('Snow3D — объёмный снег')}</option>
      <option value="fireworks" disabled={!canSelect('fireworks')}
        >{t('Fireworks — фейерверк')}</option
      >
      <option value="meteor" disabled={!canSelect('meteor')}>{t('MeteorShower — метеоры')}</option>
    </select></label
  >
  {#if snow3d}
    <label
      >{t('Интенсивность Snow3D')}<select bind:value={snow3d.effect.intensity}
        ><option value={1}>1</option><option value={2}>2</option><option value={3}>3</option
        ></select
      ></label
    >
    <StudioPopover label={t('Справка')} icon="help"
      ><p>
        {t(
          'Интенсивность меняет скорость и плотность объёмного снега. Текст и изображения остаются поверх эффекта.',
        )}
      </p></StudioPopover
    >
  {/if}
  {#if weather}
    <StudioPopover label={t('Справка')} icon="help"
      ><p>
        {t('Эффект движется на фоне; текст, часы и изображения остаются поверх него.')}
      </p></StudioPopover
    >
    <label
      >{t('Интенсивность эффекта')}<select bind:value={weather.effect.intensity}
        ><option value={1}>{t('Низкая')}</option><option value={2}>{t('Средняя')}</option><option
          value={3}>{t('Высокая')}</option
        ></select
      ></label
    >
    <label
      ><input
        type="checkbox"
        checked={weather.effect.dynamic}
        onchange={(event) => dynamic(event.currentTarget.checked)}
      />{t('Управлять эффектом от датчика')}</label
    >
    {#if binding || weather.effect.dynamic}
      <fieldset disabled={!weather.effect.dynamic}>
        <legend>{t('Датчик эффекта')}</legend>
        <label
          >{t('Источник эффекта')}<select
            value={binding ? bindingKey(binding) : ''}
            onchange={(event) => {
              if (!chooseSensor(event.currentTarget.value))
                event.currentTarget.value = binding ? bindingKey(binding) : '';
            }}
          >
            {#if !binding}<option value="" disabled>{t('Выберите датчик')}</option>{/if}
            {#each sources as source (sourceKey(source))}<option
                value={sourceKey(source)}
                disabled={!canChoose(source)}>{source.label} · {source.unit}</option
              >{/each}
            {#if binding && !sources.some((source) => sourceKey(source) === bindingKey(binding))}<option
                value={binding ? bindingKey(binding) : ''}
                >{t('Недоступен:')} {binding.sensor_id}</option
              >{/if}
          </select></label
        >
        {#if binding}
          <StudioPopover label={t('Справка')} icon="help"
            ><p>
              {t('Единицы порогов:')}
              {binding.unit === 'celsius'
                ? displaySuffix
                : binding.unit === 'percent'
                  ? '%'
                  : 'RPM'}{t('. Диапазон:')}
              {binding.unit === 'celsius'
                ? temperatureFromCelsius(0, displayUnit)
                : 0}–{binding.unit === 'celsius'
                ? temperatureFromCelsius(rangeMax, displayUnit)
                : rangeMax}.
            </p></StudioPopover
          >
          <label
            >{t('Нижний порог эффекта')}{#if binding.unit === 'celsius'}<TemperatureInput
                bind:value={binding.low}
                min={0}
                max={rangeMax}
                step={1}
                invalid={!Number.isInteger(binding.low)}
              />{:else}<input
                type="number"
                step="1"
                min="0"
                max={rangeMax}
                bind:value={binding.low}
              />{/if}</label
          >
          <label
            >{t('Верхний порог эффекта')}{#if binding.unit === 'celsius'}<TemperatureInput
                bind:value={binding.high}
                min={0}
                max={rangeMax}
                step={1}
                invalid={!Number.isInteger(binding.high)}
              />{:else}<input
                type="number"
                step="1"
                min="0"
                max={rangeMax}
                bind:value={binding.high}
              />{/if}</label
          >
          {#if weather.effect.kind === 'snow' && binding.low === binding.high}<StudioPopover
              label={t('Справка')}
              icon="help"
              ><p>
                {t(
                  'При точном совпадении показания с порогом Snow использует минимальную скорость.',
                )}
              </p></StudioPopover
            >{/if}
        {/if}
      </fieldset>
      {#if weather.effect.dynamic && !binding}<p role="status">
          {choosingSource
            ? '' + t('Чтение источника CPU Load…') + ''
            : sourceError
              ? t('Источник CPU Load не прочитан: {0}. Выберите датчик вручную.', sourceError)
              : '' + t('Выберите источник эффекта: CPU Load недоступен или неоднозначен.') + ''}
        </p>{/if}
      {#if (weather.effect.retained_sensors ?? []).length >= 63}<p role="status">
          {t('Сохранено 64 диапазона: можно выбирать уже настроенные источники.')}
        </p>{/if}
      {#if weather.effect.dynamic && unavailable}<p role="status">
          {t('Источник недоступен: используется сохранённая интенсивность.')}
        </p>{/if}
      {#if !thresholdsValid}<p role="status">
          {t('Задайте целые пороги; нижний не должен превышать верхний.')}
        </p>{/if}
    {/if}
  {/if}
</fieldset>
