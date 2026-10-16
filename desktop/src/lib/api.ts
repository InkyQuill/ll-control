import { invoke } from '@tauri-apps/api/core';
export type Unit = 'celsius' | 'percent' | 'bytes' | 'rpm' | 'watts' | 'volts' | 'hertz' | 'bytes_per_second' | 'frames_per_second';
export interface Sensor { id: string; label: string; unit: Unit; value: number | null; status: string; age_ms: number | null }
export interface Device { id: string; display_name: string }
export interface Style { font_asset?:string|null; line_box?:boolean; max_width?:number|null; line_height?:number|null; vertical_align?:'top'|'center'|'bottom'; font?: 'sans' | 'serif' | 'mono' | 'noto_sans_tc_regular' | 'noto_sans_tc_bold' | 'abril_fatface' | 'caprasimo' | 'cubic11'; opacity?:number; shadow?: {x:number;y:number;blur:number;color:number[];opacity:number}|null; x: number; y: number; size: number; color: number[]; align: 'left' | 'center' | 'right' }
export interface ThemeSensor { sensor_id:string; label:string; unit:Unit; temperature_unit:'system' | 'celsius' | 'fahrenheit'; decimals:number }
export interface EffectSensor {sensor_id:string;unit:Unit;low:number;high:number}
export interface WeatherPreset { kind:'rain'|'snow'; intensity:number; dynamic:boolean; sensor:EffectSensor|null; retained_sensors?:EffectSensor[]; seed:number }
export interface WeatherEffect extends WeatherPreset {presets?:WeatherPreset[]}
export type RgbPair=[[number,number,number],[number,number,number]];
export interface MeteorSensor {binding:EffectSensor;low_colors:RgbPair;high_colors:RgbPair}
export interface MeteorEffect {speed:number;colors:RgbPair;dynamic:boolean;sensor:MeteorSensor|null;retained_sensors?:MeteorSensor[];seed:number}
export interface Snow3dEffect {intensity:number;seed:number}
export type RgbSix=[[number,number,number],[number,number,number],[number,number,number],[number,number,number],[number,number,number],[number,number,number]];
export interface FireworksSensor {binding:EffectSensor;low_colors:RgbSix;high_colors:RgbSix}
export interface FireworksEffect {intensity:number;colors:RgbSix;dynamic:boolean;sensor:FireworksSensor|null;retained_sensors?:FireworksSensor[];seed:number}
export function fireworksColors():RgbSix {return [[0,255,0],[0,0,255],[128,0,255],[255,0,65],[255,215,0],[130,173,237]];}
export type Layer =
  | {kind:'fireworks';effect:FireworksEffect}
  | {kind:"snow3d";effect:Snow3dEffect}
  | {kind:'meteor';effect:MeteorEffect}
  | {kind:'weather';effect:WeatherEffect}
  | { kind: 'sensor_cycle'; parts?: {label:Style;unit:Style}|null; sensors: ThemeSensor[]; interval_seconds:number; style:Style }
  | { kind: 'text'; text: string; style: Style }
  | { kind: 'sensor'; parts?: {label:Style;unit:Style}|null; sensor_id: string; label: string; unit: Unit; temperature_unit: 'system' | 'celsius' | 'fahrenheit'; decimals: number; style: Style }
  | { kind: 'clock'; format?: 'time' | 'time_without_period' | 'date' | 'period'; twenty_four_hour: boolean; show_seconds: boolean; style: Style }
  | { kind: 'image'; watermark_position?: {horizontal:number;vertical:number}|null; shadow?: Style['shadow']; asset: string; x: number; y: number; width: number; height: number; fit: 'crop' | 'contain' | 'contain_native' | 'stretch'; opacity: number };
export type BackgroundMode = 'solid' | 'horizontal' | 'vertical' | 'radial';
export interface GradientColors { colors: [number[], number[]]; flip: boolean }
export interface BackgroundFill { mode: BackgroundMode; horizontal: GradientColors; vertical: GradientColors; radial: GradientColors }
export type LcdPageKind='basic'|'rain'|'snow'|'meteor'|'snow3d'|'fireworks';
export interface SavedLcdPage {page:LcdPageKind;theme:Theme}
export interface Theme { page_cache?:SavedLcdPage[]; background_image?: {asset: string; fit: 'crop' | 'contain' | 'contain_native' | 'stretch'} | null; background_video?: string | null; background_fill?: BackgroundFill | null; version: number; name: string; background: number[]; layers: Layer[] }
export interface LcdState { persistence?: {pending:boolean;saved:boolean;last_error:string|null}|null; source: string; last_error: string | null; requested: { brightness_percent: number; rotation_degrees: number; mode: string } }
export async function rpc<T>(method: string, params: object = {}): Promise<T> {
  return invoke<T>('daemon_call', { method, params });
}
export function message(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error) return String(error.message);
  return String(error);
}
export function hex(rgb: number[]): string { return '#' + rgb.map(n => n.toString(16).padStart(2, '0')).join(''); }
export function rgb(value: string): number[] { return [1, 3, 5].map(i => parseInt(value.slice(i, i + 2), 16)); }
export const style = (y: number): Style => ({ line_box: true, x: 240, y, size: 36, color: [235, 245, 255], align: 'center' });
export const initialTheme = (name = 'My screen'): Theme => ({ version: 1, name, background: [12, 18, 30], layers: [
  { kind: 'text', text: 'L-Connect Linux', style: style(110) },
  { kind: 'clock', twenty_four_hour: true, show_seconds: true, style: style(300) }
] });
export type CoolingPolicy =
  | { mode: 'fixed' | 'motherboard'; percent: number }
  | { mode: 'fixed_rpm'; rpm: number }
  | { mode: 'curve' | 'fan_curve'; sensor_id: string; points: { celsius: number; percent: number }[]; hysteresis_c: number; fallback_percent: number }
  | { mode: 'curve_rpm'; sensor_id: string; points: { celsius: number; rpm: number }[]; hysteresis_c: number; fallback_percent: number };
export interface CoolingChannel { requested: CoolingPolicy | null; applied_percent: number | null; motherboard: boolean; fallback_active: boolean; last_error: string | null }
export interface CoolingState { fan: CoolingChannel; pump: CoolingChannel }
export function coolingPreset(target: 'fan' | 'pump', preset: string, sensorId: string): Promise<CoolingPolicy> {
  return invoke('cooling_preset', { target, preset, sensorId });
}

export async function importImageAsset(file: File): Promise<string> {
  if (file.size > 16 * 1024 * 1024) throw new Error('Изображение превышает 16 MiB');
  return invoke<string>('import_image', { name: file.name, bytes: Array.from(new Uint8Array(await file.arrayBuffer())) });
}

export type ProfileGroup = 'screen' | 'fan' | 'pump';
export type ProfileContent = { kind: 'screen'; value: Theme } | { kind: 'screen_media'; value: string } | { kind: 'fan' | 'pump'; value: CoolingPolicy };
export function profileGroup(content: ProfileContent): ProfileGroup { return content.kind === 'screen_media' ? 'screen' : content.kind; }
export interface NamedProfile { slot: number; name: string; content: ProfileContent }
export interface ProfileLibrary { version: number; revision: number; profiles: NamedProfile[] }
export type ProfileEdit = { action: 'reset_group'; group: ProfileGroup; sensor_id: string | null } | { action: 'reset'; group: ProfileGroup; slot: number; sensor_id: string | null } | { action: 'save'; profile: NamedProfile } | { action: 'rename'; group: ProfileGroup; slot: number; name: string } | { action: 'delete'; group: ProfileGroup; slot: number };


export type CpuSourceField = 'temperature' | 'clock' | 'load' | 'power' | 'fan' | 'voltage';
export type GpuSourceField = CpuSourceField | 'memory_used';
export type DefaultSensorRole = `cpu_${CpuSourceField}` | `gpu_${GpuSourceField}`;
export interface SensorDefaults { version: number; revision: number; cpu: Record<CpuSourceField, string | null>; gpu: (Record<GpuSourceField, string | null> & { device_id: string }) | null }
export interface SensorDefaultCandidate { role: DefaultSensorRole; gpu_id: string | null; reading: Sensor }
export interface SensorDefaultsState { config: SensorDefaults; recommended: SensorDefaults; candidates: SensorDefaultCandidate[]; values: { role: DefaultSensorRole; sensor_id: string | null; reading: Sensor | null }[] }

export interface PreparedClipInfo { is_default?: boolean; key: string; name: string; fps: number; frames: number; encoded_bytes: number }

export type CollectionContext = 'image' | 'background' | 'watermark';
export type CollectionAsset = { kind: 'image' | 'video'; key: string };
export interface ScreenCollections { version: number; revision: number; images: string[]; backgrounds: CollectionAsset[]; watermarks: string[] }
export interface ScreenCollectionsState { collections: ScreenCollections | null; persistence: { pending: boolean; saved: boolean; last_error: string | null } }

export interface ScreenResetReceipt { operation_id: string; profile_revision: number; collection_revision: number }
export interface ScreenResetReadback { receipt: ScreenResetReceipt | null; persistence: ScreenCollectionsState['persistence']; reapply_available: boolean; lcd: LcdState & { generation: number } }

export type SpecKind = 'cpu' | 'gpu' | 'memory' | 'monitors' | 'motherboard' | 'storage';
export type SpecValue = {kind:'text';value:string} | {kind:'count'|'bytes'|'hertz';value:number};
export interface SpecField {key:string;value:SpecValue|null;source:string;unavailable_reason:string|null}
export interface SpecItem {id:string;title:string;fields:SpecField[]}
export interface SpecGroup {kind:SpecKind;items:SpecItem[];unavailable_reason:string|null}
export interface SystemSpec {version:number;collected_at_unix_ms:number;groups:SpecGroup[]}

export interface DisplaySettings {version:number;revision:number;temperature_unit:'celsius'|'fahrenheit';interface_language?:'ru-RU'|'en-US'}

export interface FpsSourceSettings {version:1;revision:number;enabled:boolean;log_path:string|null}
