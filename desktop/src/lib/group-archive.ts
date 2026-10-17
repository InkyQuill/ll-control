import type {ScreenRequirements,ScreenOptions} from './windows-screen';
import type {WindowsOptions,WindowsLightingReview} from './windows-control';
import type {DeviceCoolingMode} from './cooling-modes';
import {translate,type InterfaceLanguage} from './localization';
import type {LcdState, CoolingState, CoolingPolicy} from './api';
export type FullGroup='screen'|'fan_and_lighting';
export type GroupMode='import'|'export'|'reset'|'windows'|'windows_screen';
export interface GroupTarget {id:string;dev:string;group:FullGroup;mode:GroupMode}
export interface GroupSummary {
  version:number;slots:{group:string;slot:number;name:string}[];
  lcd:{brightness_percent:number;rotation_degrees:number;mode:string}|null;
  theme_name:string|null;media_key:string|null;fan_manual_percent:number|null;
  pump_manual:{percent:number;source:string}|null;
  fan_led_mode:string|null;pump_led_mode:string|null;
  cooling:{fan:CoolingPolicy|null;pump:CoolingPolicy|null;fan_mode?:string;pump_mode?:string}|null;
  cooling_modes?:{fan:DeviceCoolingMode[];pump:DeviceCoolingMode[]}|null;
  dynamic_rgb:{sensor_id:string;unit:string;low_value:number;high_value:number}|null;
}
export interface GroupReadback {dev:string;group:FullGroup;operation_id:string;receipt:string;selected_content_current:boolean;summary:GroupSummary;runtime:{revision:string|null;lcd:LcdState|null;cooling:CoolingState|null;recovery_required:boolean|null;last_error:string|null}}
export type GroupPhase='choosing'|'bindings'|'uploading'|'inspecting'|'prepared'|'packing'|'downloading'|'applying'|'recovering'|'cancelling'|'cancelled'|'complete'|'failed'|'uncertain';
export interface GroupStatus extends GroupTarget {windows_screen_requirements?:ScreenRequirements|null;windows_screen_options?:ScreenOptions|null;windows_source?:{source_version:string;backup_sha256:string;controllers:{record_index:number;profile_sha256:string}[]}|null;windows_options?:WindowsOptions|null;windows_lighting?:WindowsLightingReview|null;sensor_id?:string|null;snapshot?:{expected:string}|null;phase:GroupPhase;transferred:number;total:number;file_name:string|null;message:string|null;message_code?:string|null;summary:GroupSummary|null;readback:GroupReadback|null}
export const groupNames:Record<FullGroup,string>={screen:'Экран',fan_and_lighting:'Вентиляторы и подсветка'};
export const groupPhases:Record<GroupPhase,string>={choosing:'Выбор файла',bindings:'Выбор исходного контроллера и датчиков',uploading:'Загрузка архива',inspecting:'Проверка содержимого',prepared:'Архив проверен, ожидает применения',packing:'Подготовка архива',downloading:'Сохранение файла',applying:'Применение группы',recovering:'Чтение результата',cancelling:'Отмена передачи',cancelled:'Передача отменена',complete:'Операция завершена',failed:'Операция не выполнена',uncertain:'Результат требует проверки'};
export const groupIdle=(phase:GroupPhase)=>['bindings','prepared','cancelled','complete','failed','uncertain'].includes(phase);
export const groupPendingKey='lconnect.group.pending';
export function readGroupPending():GroupTarget|null {
  try {
    const raw=typeof localStorage==='undefined'?null:localStorage.getItem(groupPendingKey);
    if(!raw)return null;
    const v:unknown=JSON.parse(raw);
    if(typeof v!=='object'||v===null)return null;
    const p=v as Partial<GroupTarget>;
    return typeof p.id==='string'&&/^[a-f0-9]{32}$/.test(p.id)&&typeof p.dev==='string'&&p.dev.length>0&&p.dev.length<=512&&(p.group==='screen'||p.group==='fan_and_lighting')&&(p.mode==='import'||p.mode==='export'||(p.mode==='reset'||p.mode==='windows')&&p.group==='fan_and_lighting'||p.mode==='windows_screen'&&p.group==='screen')?p as GroupTarget:null;
  }catch{return null;}
}

export function policySummary(p:CoolingPolicy|null,language:InterfaceLanguage='en-US'):string {
  if(!p)return translate('не задана',language);
  switch(p.mode){
    case 'fixed':return `${p.percent}%`;
    case 'motherboard':return translate('Материнская плата',language);
    case 'fixed_rpm':return `${p.rpm} RPM`;
    case 'curve':case 'fan_curve':return translate('Кривая PWM · {0} · точек: {1}',language,p.sensor_id,p.points.length);
    case 'curve_rpm':return translate('Кривая RPM · {0} · точек: {1}',language,p.sensor_id,p.points.length);
  }
}
