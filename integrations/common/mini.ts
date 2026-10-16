/** Shared, pure shell model. No GUI, process, hardware access or resource rewriting. */
export type Language = 'en-US' | 'ru-RU';
export type Kind = 'cpu_load'|'cpu_power'|'cpu_temperature'|'cpu_voltage'|'gpu_load'|'gpu_power'|'gpu_temperature'|'gpu_voltage'|'network'|'ram'|'fps';
export interface Tile {kind:Kind;source:string|null}
export interface Config {version:1;slots:(Tile|null)[]}
type ObjectValue=Record<string,unknown>;
const object=(value:unknown):value is ObjectValue=>typeof value==='object'&&value!==null&&!Array.isArray(value);
const only=(value:ObjectValue,keys:string[])=>Object.keys(value).every(key=>keys.includes(key));
const units:Partial<Record<Kind,string>>={cpu_load:'percent',cpu_power:'watts',cpu_temperature:'celsius',cpu_voltage:'volts',gpu_load:'percent',gpu_power:'watts',gpu_temperature:'celsius',gpu_voltage:'volts',fps:'frames_per_second'};
export const kinds:Kind[]=['cpu_load','cpu_power','cpu_temperature','cpu_voltage','gpu_load','gpu_power','gpu_temperature','gpu_voltage','network','ram','fps'];
const labels:Record<Language,Record<Kind,string>>={
 'en-US':{cpu_load:'CPU load',cpu_power:'CPU power',cpu_temperature:'CPU temperature',cpu_voltage:'CPU voltage',gpu_load:'GPU load',gpu_power:'GPU power',gpu_temperature:'GPU temperature',gpu_voltage:'GPU voltage',network:'Network',ram:'RAM',fps:'Average FPS'},
 'ru-RU':{cpu_load:'Загрузка CPU',cpu_power:'Мощность CPU',cpu_temperature:'Температура CPU',cpu_voltage:'Напряжение CPU',gpu_load:'Загрузка GPU',gpu_power:'Мощность GPU',gpu_temperature:'Температура GPU',gpu_voltage:'Напряжение GPU',network:'Сеть',ram:'Оперативная память',fps:'Средний FPS'}
};
const messages:Record<Language,Record<string,string>>={
 'en-US':{empty:'Empty slot',unavailable:'No current data',offline:'Daemon unavailable',invalid:'Unsupported or invalid widget settings. Reset explicitly to edit.',source:'Source',shared:'Shared CPU/GPU selection',unselected:'No FPS source selected',total:'All interfaces (including virtual)',missing:'Saved source unavailable',reading:'Reading',reset:'Reset slots',fps:'FPS requires an active MangoHud per-frame log and an explicit source selection.',configuration:'Six readings',slot:'Slot'},
 'ru-RU':{empty:'Пустой слот',unavailable:'Нет актуальных данных',offline:'Daemon недоступен',invalid:'Неподдерживаемые или повреждённые настройки виджета. Для редактирования явно сбросьте их.',source:'Источник',shared:'Общий выбор CPU/GPU',unselected:'Источник FPS не выбран',total:'Все интерфейсы (включая виртуальные)',missing:'Сохранённый источник недоступен',reading:'Показание',reset:'Сбросить слоты',fps:'Для FPS нужны активный покадровый лог MangoHud и явный выбор источника.',configuration:'Шесть показаний',slot:'Слот'}
};
export function text(key:string,language:Language='en-US'):string{return messages[language]?.[key]??messages['en-US'][key]??key;}
export function label(kind:Kind,language:Language='en-US'):string{return labels[language]?.[kind]??labels['en-US'][kind]??kind;}
export function defaultConfig():Config{return {version:1,slots:(['cpu_load','cpu_power','cpu_voltage','gpu_load','gpu_power','gpu_voltage'] as Kind[]).map(kind=>({kind,source:null}))};}
export function parseConfig(raw:unknown):Config|null {
 try {
  if(typeof raw==='string'&&raw.length>16384)return null;
  const value:unknown=typeof raw==='string'?JSON.parse(raw):raw;
  if(!object(value)||!only(value,['version','slots'])||value.version!==1||!Array.isArray(value.slots)||value.slots.length!==6)return null;
  for(const slot of value.slots) {
   if(slot===null)continue;
   if(!object(slot)||!only(slot,['kind','source'])||!kinds.includes(slot.kind as Kind))return null;
   if(slot.source!==null&&(typeof slot.source!=='string'||slot.source.length===0||slot.source.length>1024||/[\u0000-\u001f\u007f]/.test(slot.source)))return null;
   if(slot.kind==='ram'&&slot.source!==null)return null;
   if(slot.kind==='fps'&&slot.source!==null&&!fpsId(String(slot.source)))return null;
   if(slot.kind==='network'&&typeof slot.source==='string'&&!slot.source.startsWith('network:'))return null;
  }
  return value as unknown as Config;
 }catch{return null;}
}
function dataOf(frame:unknown):ObjectValue|null {
 if(!object(frame)||frame.version!==1||frame.available!==true||!object(frame.data))return null;
 const d=frame.data;
 if(!Array.isArray(d.sensors)||d.sensors.length>8192||!object(d.defaults)||!Array.isArray(d.defaults.candidates)||!Array.isArray(d.defaults.values)||!object(d.display))return null;
 if(!['celsius','fahrenheit'].includes(String(d.display.temperature_unit)))return null;
 return d;
}
export function languageOf(frame:unknown,fallback:Language='en-US'):Language {
 const data=dataOf(frame);if(!data)return fallback;
 const language=(data.display as ObjectValue).interface_language;
 return language==='ru-RU'?'ru-RU':language==='en-US'?'en-US':fallback;
}
function fpsId(id:string):boolean{return /^mangohud:[0-9a-f]+:[0-9a-f]+:[0-9a-f]+:logged_frame_mean$/.test(id);}
export interface Source {id:string;label:string}
export function sources(kind:Kind,frame:unknown):Source[] {
 const data=dataOf(frame);if(!data)return [];
 const rows=kind==='fps'?(data.sensors as unknown[]).filter((s):s is ObjectValue=>object(s)&&s.unit==='frames_per_second'&&typeof s.id==='string'&&fpsId(s.id)&&typeof s.label==='string').map(s=>({id:s.id as string,label:s.label as string})):
 kind==='network'?(data.sensors as unknown[]).filter((s):s is ObjectValue=>object(s)&&s.unit==='bytes_per_second'&&typeof s.id==='string'&&s.id.startsWith('network:')&&s.id.endsWith(':rx_bytes_per_second')).map(s=>({id:(s.id as string).slice(0,-':rx_bytes_per_second'.length),label:typeof s.label==='string'?s.label:s.id as string})):
 ((data.defaults as ObjectValue).candidates as unknown[]).filter((c):c is ObjectValue=>object(c)&&c.role===kind&&object(c.reading)&&c.reading.unit===units[kind]).map(c=>{const s=c.reading as ObjectValue;return {id:s.id,label:s.label};}).filter((s):s is Source=>typeof s.id==='string'&&typeof s.label==='string');
 const counts=new Map<string,number>();for(const row of rows)counts.set(row.id,(counts.get(row.id)??0)+1);
 return rows.filter(row=>counts.get(row.id)===1);
}
interface Reading {id:string;unit:string;value:number;label?:string}
function reading(data:ObjectValue,id:string,unit:string,elapsed:number):Reading|null {
 const rows=(data.sensors as unknown[]).filter((s):s is ObjectValue=>object(s)&&s.id===id);
 if(rows.length!==1)return null;
 const s=rows[0];
 if(unit==='frames_per_second'&&(!(typeof s.value==='number')||s.value<=0))return null;
 if(s.unit!==unit||s.status!=='valid'||typeof s.value!=='number'||!Number.isFinite(s.value)||typeof s.age_ms!=='number'||!Number.isFinite(s.age_ms)||s.age_ms<0||s.age_ms+elapsed>3000)return null;
 return s as unknown as Reading;
}
function format(s:Reading,display:ObjectValue,language:Language):string {
 let value=s.value,unit=s.unit,decimals=1,suffix='';
 if(unit==='celsius'){if(display.temperature_unit==='fahrenheit'){value=value*9/5+32;suffix=' °F';}else suffix=' °C';}
 else if(unit==='bytes'){value/=1024**3;suffix=' GiB';}
 else if(unit==='percent')suffix='%';
 else if(unit==='watts')suffix=' W';
 else if(unit==='volts'){suffix=' V';decimals=2;}
 else if(unit==='bytes_per_second')suffix=' B/s';
 else if(unit==='frames_per_second')suffix=' FPS';
 let number=value.toFixed(decimals);if(language==='ru-RU')number=number.replace('.',',');return number+suffix;
}
export interface Card {title:string;values:string[];available:boolean;sourceLabel:string}
export function render(frame:unknown,rawConfig:unknown,elapsedMs=0,fallback:Language='en-US'):{language:Language;validConfig:boolean;status:string;cards:Card[]} {
 const language=languageOf(frame,fallback),config=parseConfig(rawConfig),data=dataOf(frame);
 const elapsed=Number.isFinite(elapsedMs)&&elapsedMs>=0?elapsedMs:Infinity;
 const cards=(config?.slots??Array(6).fill(null)).map((tile:Tile|null):Card=>{
  const result:Card={title:tile?label(tile.kind,language):text('empty',language),values:[],available:false,sourceLabel:''};
  if(!tile||!data||elapsed>3000)return result;
  const display=data.display as ObjectValue;let rows:Reading[]=[];
  if(tile.kind==='ram') {
   const load=reading(data,'proc:memory:utilization','percent',elapsed),used=reading(data,'proc:memory:used','bytes',elapsed);if(load&&used)rows=[load,used];
  }else if(tile.kind==='network') {
   const prefix=tile.source??'network:total';const rx=reading(data,prefix+':rx_bytes_per_second','bytes_per_second',elapsed),tx=reading(data,prefix+':tx_bytes_per_second','bytes_per_second',elapsed);if(rx&&tx)rows=[rx,tx];
  }else {
   const selections=((data.defaults as ObjectValue).values as unknown[]).filter((v):v is ObjectValue=>object(v)&&v.role===tile.kind);
   const id=tile.source??(tile.kind!=='fps'&&selections.length===1?selections[0].sensor_id:null),unit=units[tile.kind];
   if(typeof id==='string'&&unit&&sources(tile.kind,frame).some(s=>s.id===id)) {const s=reading(data,id,unit,elapsed);if(s)rows=[s];}
  }
  if(rows.length){result.available=true;result.sourceLabel=rows[0].label??'';result.values=rows.map((row,index)=>(tile.kind==='network'?(index===0?'↓ ':'↑ '):'')+format(row,display,language));}
  return result;
 });
 return {language,validConfig:!!config,status:!config?text('invalid',language):!data||elapsed>3000?text('offline',language):'',cards};
}
