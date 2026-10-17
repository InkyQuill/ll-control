<script lang="ts">
  import {t} from './i18n.svelte';
  import {onDestroy,untrack} from 'svelte';
  import {rpc,message,hex,rgb,type Theme,type Sensor,type MeteorSensor,type FireworksSensor,type EffectSensor,type SensorDefaultsState,fireworksColors} from './api';
  import {weatherRangeValid,weatherRangeMaximum,initialWeatherCpuLoad} from './weather-settings';
  let {theme=$bindable(),sensors,disabled=false}:{theme:Theme;sensors:Sensor[];disabled?:boolean}=$props();
  const active=$derived(theme.layers.find(layer=>layer.kind==='meteor'||layer.kind==='fireworks'));
  const family=$derived(active?.kind==='fireworks'?'фейерверка':'метеоров');
  const indices=$derived(Array.from({length:active?.effect.colors.length??0},(_,index)=>index));
  const source=$derived(active?.effect.sensor);
  const binding=$derived(source?.binding);
  const sources=$derived(sensors.filter(sensor=>['celsius','percent','rpm'].includes(sensor.unit)));
  const valid=$derived(weatherRangeValid(binding));
  const maximum=$derived(weatherRangeMaximum(binding));
  const unavailable=$derived(binding&&!sources.some(sensor=>sensor.id===binding.sensor_id&&sensor.unit===binding.unit&&sensor.status==='valid'&&sensor.value!==null&&sensor.age_ms!==null&&sensor.age_ms<=3000));
  let request=0,choosing=$state(false),sourceError=$state('');
  $effect(()=>{void theme;untrack(()=>{request++;choosing=false;sourceError='';});});
  onDestroy(()=>{request++;});
  function key(sensor:{id:string;unit:Sensor['unit']}) {return JSON.stringify([sensor.id,sensor.unit]);}
  function savedKey(source:{binding:EffectSensor}) {return key({id:source.binding.sensor_id,unit:source.binding.unit});}
  function canChoose(sensor:Sensor) {
    if(!active||!source||savedKey(source)===key(sensor))return true;
    return valid&&((active.effect.retained_sensors??[]).some(source=>savedKey(source)===key(sensor))||(active.effect.retained_sensors??[]).length<63);
  }
  function createMeteorSource(binding:EffectSensor):MeteorSensor {return {binding,low_colors:[[0,255,0],[0,0,255]],high_colors:[[0,255,0],[0,0,255]]};}
  function createFireworksSource(binding:EffectSensor):FireworksSensor {return {binding,low_colors:fireworksColors(),high_colors:fireworksColors()};}
  type ColourSource={binding:EffectSensor};
  function replaceSource<T extends ColourSource>(effect:{sensor:T|null;retained_sensors?:T[]},identity:string,fresh:()=>T) {
    const previous=effect.sensor,retained=effect.retained_sensors??[],saved=retained.find(source=>savedKey(source)===identity);
    effect.retained_sensors=retained.filter(source=>savedKey(source)!==identity);
    if(previous)effect.retained_sensors.push(previous);
    effect.sensor=saved??fresh();
  }
  function choose(value:string) {
    const sensor=sources.find(sensor=>key(sensor)===value);
    if(!active||!sensor||!canChoose(sensor))return false;
    if(source&&savedKey(source)===value)return true;
    request++;choosing=false;sourceError='';
    const binding={sensor_id:sensor.id,unit:sensor.unit,low:sensor.unit==='rpm'?800:30,high:sensor.unit==='rpm'?1200:60};
    if(active.kind==='meteor')replaceSource(active.effect,value,()=>createMeteorSource(binding));
    else replaceSource(active.effect,value,()=>createFireworksSource(binding));
    return true;
  }
  async function dynamic(enabled:boolean) {
    if(!active)return;
    const currentRequest=++request,owner=theme,captured=active,effect=active.effect;
    effect.dynamic=enabled;choosing=false;sourceError='';
    if(!enabled||effect.sensor)return;
    choosing=true;
    const current=()=>currentRequest===request&&theme===owner&&active?.effect===effect&&effect.dynamic&&!effect.sensor;
    try {
      const state=await rpc<SensorDefaultsState>('get_sensor_defaults');if(!current())return;
      const proposed=initialWeatherCpuLoad(state);
      if(proposed) {
        const identity=JSON.stringify([proposed.sensor_id,proposed.unit]);
        if(captured.kind==='meteor')replaceSource(captured.effect,identity,()=>createMeteorSource(proposed));
        else replaceSource(captured.effect,identity,()=>createFireworksSource(proposed));
      }
    }catch(error){if(current())sourceError=message(error);}
    finally{if(currentRequest===request)choosing=false;}
  }
  function color(value:string):[number,number,number] {const c=rgb(value);return [c[0],c[1],c[2]];}
</script>
{#if active}
<fieldset disabled={disabled}>
  <legend>{active.kind==='fireworks'?"" + t("Фейерверк") + "":"" + t("Метеоры") + ""}</legend>
  <p>{t("Текст, часы и изображения остаются поверх анимации.")}</p>
  <fieldset disabled={active.effect.dynamic}><legend>{t("Статические настройки")} {family}</legend>
    {#if active.kind==='meteor'}<label>{t("Скорость метеоров")}<select bind:value={active.effect.speed}><option value={1}>1</option><option value={2}>2</option><option value={3}>3</option></select></label>{:else}<label>{t("Интенсивность Fireworks")}<select bind:value={active.effect.intensity}><option value={1}>1</option><option value={2}>2</option><option value={3}>3</option></select></label>{/if}
    {#each indices as index}
      {@const colours=active.effect.colors}
      <label>{t("Цвет")} {family} {index+1}<input type="color" value={hex(colours[index])} oninput={event=>{if(active?.effect.colors===colours)colours[index]=color(event.currentTarget.value);}}/></label>
    {/each}
  </fieldset>
  <label><input type="checkbox" checked={active.effect.dynamic} onchange={event=>dynamic(event.currentTarget.checked)}/>{t("Управлять эффектом от датчика")}</label>
  {#if source||active.effect.dynamic}
  <fieldset disabled={!active.effect.dynamic}><legend>{t("Датчик")} {family}</legend>
    <label>{t("Источник эффекта")}<select value={source?savedKey(source):''} onchange={event=>{if(!choose(event.currentTarget.value))event.currentTarget.value=source?savedKey(source):'';}}>
      {#if !source}<option value="" disabled>{t("Выберите датчик")}</option>{/if}
      {#each sources as sensor}<option value={key(sensor)} disabled={!canChoose(sensor)}>{sensor.label} · {sensor.unit}</option>{/each}
      {#if source&&!sources.some(sensor=>key(sensor)===savedKey(source))}<option value={savedKey(source)}>{t("Недоступен:")} {source.binding.sensor_id}</option>{/if}
    </select></label>
    {#if source}
      <p>{t("Пороги в исходных единицах:")} {source.binding.unit==='celsius'?'°C':source.binding.unit==='percent'?'%':'RPM'}{t(". Диапазон: 0–")}{maximum}.</p>
      <label>{t("Нижний порог эффекта")}<input type="number" min="0" max={maximum} step="1" bind:value={source.binding.low}/></label>
      <label>{t("Верхний порог эффекта")}<input type="number" min="0" max={maximum} step="1" bind:value={source.binding.high}/></label>
      {#each indices as index}
        {@const paletteSource=source}
        <label>{t("Нижний цвет")} {family} {index+1}<input type="color" value={hex(paletteSource.low_colors[index])} oninput={event=>{if(source===paletteSource)paletteSource.low_colors[index]=color(event.currentTarget.value);}}/></label>
        <label>{t("Верхний цвет")} {family} {index+1}<input type="color" value={hex(paletteSource.high_colors[index])} oninput={event=>{if(source===paletteSource)paletteSource.high_colors[index]=color(event.currentTarget.value);}}/></label>
      {/each}
      {#if source.binding.low===source.binding.high}<p>{t("На равном пороге используются нижняя интенсивность и палитра; выше порога — верхние.")}</p>{/if}
    {/if}
  </fieldset>
  {/if}
  {#if active.effect.dynamic&&!source}<p role="status">{choosing?"" + t("Чтение источника CPU Load…") + "":sourceError?t("Источник CPU Load не прочитан: {0}. Выберите датчик вручную.", sourceError):"" + t("Выберите источник эффекта: CPU Load недоступен или неоднозначен.") + ""}</p>{/if}
  {#if active.effect.dynamic&&unavailable}<p role="status">{t("Источник недоступен: используются сохранённые статические настройки и цвета.")}</p>{/if}
  {#if !valid}<p role="status">{t("Задайте целые пороги; нижний не должен превышать верхний.")}</p>{/if}
  {#if (active.effect.retained_sensors??[]).length>=63}<p role="status">{t("Сохранено 64 источника: можно выбирать уже настроенные.")}</p>{/if}
</fieldset>
{/if}
<style>
  fieldset{border:1px solid var(--border);border-radius:8px;padding:16px;margin:16px 0}
  label{display:flex;gap:8px;align-items:center;margin:10px 0;flex-wrap:wrap}
  select,input{max-width:100%}
</style>
