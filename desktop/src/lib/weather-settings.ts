import type {EffectSensor,Theme} from './api';
export function weatherRangeMaximum(binding:EffectSensor|null|undefined):number {
  return binding?.unit==='rpm'?3600:100;
}
export function weatherRangeValid(binding:EffectSensor|null|undefined):boolean {
  return !binding || (Number.isInteger(binding.low) && Number.isInteger(binding.high)
    && binding.low>=0 && binding.high<=weatherRangeMaximum(binding) && binding.low<=binding.high);
}
// Keep historical v3 JSON readable in Rust; constrain new GUI edits to the
// LCDV2 controls. Explicit Apply and profile Save must not bypass input bounds.
export function assertWeatherDraft(theme:Theme):void {
  const fireworks=theme.layers.find(layer=>layer.kind==='fireworks');
  if(fireworks?.effect.dynamic && !fireworks.effect.sensor)throw new Error('Выберите источник эффекта перед сохранением или Apply.');
  if(fireworks && (!Number.isInteger(fireworks.effect.intensity)||fireworks.effect.intensity<1||fireworks.effect.intensity>3||!weatherRangeValid(fireworks.effect.sensor?.binding)))throw new Error('Проверьте интенсивность и пороги Fireworks.');
  const snow3d=theme.layers.find(layer=>layer.kind==='snow3d');
  if(snow3d&&(!Number.isInteger(snow3d.effect.intensity)||snow3d.effect.intensity<1||snow3d.effect.intensity>3))throw new Error('Проверьте интенсивность Snow3D: 1–3.');
  const meteor=theme.layers.find(layer=>layer.kind==='meteor');
  if(meteor?.effect.dynamic && !meteor.effect.sensor)throw new Error('Выберите источник эффекта перед сохранением или Apply.');
  if(meteor && (!Number.isInteger(meteor.effect.speed)||meteor.effect.speed<1||meteor.effect.speed>3||!weatherRangeValid(meteor.effect.sensor?.binding)))throw new Error('Проверьте скорость и пороги MeteorShower.');
  const weather=theme.layers.find(layer=>layer.kind==='weather');
  if(weather?.effect.dynamic && !weather.effect.sensor)throw new Error('Выберите источник эффекта перед сохранением или Apply.');
  if(weather && !weatherRangeValid(weather.effect.sensor)) {
    throw new Error('Проверьте пороги эффекта: 0–100 для °C/% и 0–3600 для RPM; нижний не должен превышать верхний.');
  }
}

// These identities are daemon-classified. Labels, ordering and GPU percentages
// cannot identify the Windows CPU Load role.
export function initialWeatherCpuLoad(state:import('./api').SensorDefaultsState):EffectSensor|null {
  const selected=state.config.cpu.load;
  const candidates=[...new Set(state.candidates.filter(candidate=>candidate.role==='cpu_load'
    && candidate.gpu_id===null && candidate.reading.unit==='percent').map(candidate=>candidate.reading.id))];
  const id=selected??(candidates.length===1?candidates[0]:null);
  return id?{sensor_id:id,unit:'percent',low:30,high:60}:null;
}
