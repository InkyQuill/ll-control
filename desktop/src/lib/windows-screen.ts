import type {Sensor,Style} from './api';
import {windowsRoles,windowsUnit,type WindowsRole,type WindowsIds} from './windows-control';
export interface ScreenRequirements {record_index:number;backup_sha256:string;bindings:{sensor_roles:WindowsRole[];fonts:string[];resources:Record<string,boolean>};payloads:{entry:string;bytes:number;kind:'image'|'video'|'unsupported'}[];deferred:string[];payloads_decoded:boolean;apply_supported:boolean}
export interface ScreenOptions {record:number;bindings_json:string;resources_json:string}
export const screenFonts=[['sans','DejaVu Sans'],['serif','DejaVu Serif'],['mono','DejaVu Sans Mono'],['noto_sans_tc_regular','Noto Sans TC Regular'],['noto_sans_tc_bold','Noto Sans TC Bold'],['abril_fatface','Abril Fatface'],['caprasimo','Caprasimo'],['cubic11','Cubic 11']] as const;
type Resource={kind:'archive';entry:string}|{kind:'existing_image'|'existing_video';key:string};
interface Bindings {name:string;sensors:Record<string,{sensor_id:string;label:string;unit:string;temperature_unit:'system';decimals:number}>;fonts:Record<string,{font:Style['font'];font_asset:string|null}>;resources:Record<string,never>}
export function screenOptions(req:ScreenRequirements|null|undefined,record:string,name:string,ids:WindowsIds,fonts:Record<string,string>,resources:Record<string,string>,sensors:Sensor[],images:string[],videos:string[],customFonts:string[]):ScreenOptions|null {
 if(!req||req.deferred.length||record!==String(req.record_index)||!name.trim()||new TextEncoder().encode(name).length>128||name.includes('\0'))return null;
 const bindings:Bindings={name,sensors:{},fonts:{},resources:{}};const used=new Set<string>();
 for(const role of req.bindings.sensor_roles){
  if(!windowsRoles.includes(role))return null;
  const id=ids[role],unit=windowsUnit(role);
  if(!id?.trim()||new TextEncoder().encode(id).length>1024||/\p{Cc}/u.test(id)||used.has(id)||sensors.some(s=>s.id===id&&s.unit!==unit))return null;
  used.add(id);bindings.sensors[role]={sensor_id:id,label:role,unit,temperature_unit:'system',decimals:unit==='rpm'?0:1};
 }
 for(const family of req.bindings.fonts){
  const value=fonts[family];
  if(screenFonts.some(([key])=>key===value))bindings.fonts[family]={font:value as Style['font'],font_asset:null};
  else if(value?.startsWith('asset:')&&customFonts.includes(value.slice(6)))bindings.fonts[family]={font:'sans',font_asset:value.slice(6)};
  else return null;
 }
 const mapped=new Map<string,Resource>();
 for(const [reference,imageOnly] of Object.entries(req.bindings.resources)){
  const value=Object.hasOwn(resources,reference)?resources[reference]:'',kind=value.slice(0,value.indexOf(':')),key=value.slice(value.indexOf(':')+1);
  if(kind==='archive'&&req.payloads.some(p=>p.entry===key&&(p.kind==='image'||!imageOnly&&p.kind==='video')))mapped.set(reference,{kind:'archive',entry:key});
  else if(kind==='image'&&images.includes(key))mapped.set(reference,{kind:'existing_image',key});
  else if(kind==='video'&&!imageOnly&&videos.includes(key))mapped.set(reference,{kind:'existing_video',key});
  else return null;
 }
 const result={record:req.record_index,bindings_json:JSON.stringify(bindings),resources_json:JSON.stringify(Object.fromEntries(mapped))};
 return [result.bindings_json,result.resources_json].every(s=>new TextEncoder().encode(s).length<=64*1024)?result:null;
}
export function restoreScreenOptions(options:ScreenOptions){
 const bindings=JSON.parse(options.bindings_json) as Bindings;const mapped=JSON.parse(options.resources_json) as Record<string,Resource>;
 return {name:bindings.name,ids:Object.fromEntries(Object.entries(bindings.sensors).map(([role,value])=>[role,value.sensor_id])),fonts:Object.fromEntries(Object.entries(bindings.fonts).map(([family,value])=>[family,value.font_asset?`asset:${value.font_asset}`:value.font??'sans'])),resources:Object.fromEntries(Object.entries(mapped).map(([ref,value])=>[ref,value.kind==='archive'?`archive:${value.entry}`:`${value.kind==='existing_image'?'image':'video'}:${value.key}`]))};
}
