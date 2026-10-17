import type { Layer, Style } from './api';

export interface LayerBounds { x: number; y: number; width: number; height: number }
export interface EditorPreview { jpeg: number[]; background?:number[]; layer_images?:number[][]; layers: (LayerBounds | null)[] }
export type QuarterTurn = 0 | 90 | 180 | 270;

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value));
const integer = (value: number, low: number, high: number) => Math.round(clamp(value, low, high));

/** Convert a point in a displayed square back to unrotated native LCD coordinates. */
export function nativePoint(x: number, y: number, side: number, rotation: QuarterTurn): [number, number] {
  const px = x * 480 / side, py = y * 480 / side;
  switch (rotation) {
    case 90: return [py, 480 - px];
    case 180: return [480 - px, 480 - py];
    case 270: return [480 - py, px];
    default: return [px, py];
  }
}

export function editableLayer(layer: Layer): boolean {
  return 'style' in layer || layer.kind === 'image' || layer.kind === 'divider';
}

function textStyles(layer: Layer): Style[] {
  if (!('style' in layer)) return [];
  if ((layer.kind === 'sensor' || layer.kind === 'sensor_cycle') && layer.parts) {
    return [layer.style, layer.parts.label, layer.parts.unit];
  }
  return [layer.style];
}

/** Mutate a plain draft copy; keep sensor part offsets and all unrelated imported fields. */
export function translateLayer(layer: Layer, dx: number, dy: number): void {
  const styles = textStyles(layer);
  if (styles.length) {
    dx = integer(dx, -Math.min(...styles.map(s => s.x)), 479 - Math.max(...styles.map(s => s.x)));
    dy = integer(dy, -Math.min(...styles.map(s => s.y)), 479 - Math.max(...styles.map(s => s.y)));
    for (const style of styles) { style.x += dx; style.y += dy; }
  } else if (layer.kind === 'image' && layer.watermark_position) {
    // Windows watermark positions are percentage centers, with 0 meaning 22px.
    // Retain that representation and its quantization instead of losing it on drag.
    const shift = (value: number, delta: number) => {
      if (Math.abs(delta) < 2.4) return value;
      const center = value === 0 ? 22 : Math.round(value * 4.8);
      const next = clamp(center + delta, 0, 480);
      return Math.abs(next - 22) < Math.abs(next - Math.round(next / 4.8) * 4.8)
        ? 0 : integer(next / 4.8, 1, 100);
    };
    layer.watermark_position.horizontal = shift(layer.watermark_position.horizontal, dx);
    layer.watermark_position.vertical = shift(layer.watermark_position.vertical, dy);
  } else if (layer.kind === 'image') {
    layer.x = integer(layer.x + dx, 0, 480);
    layer.y = integer(layer.y + dy, 0, 480);
  } else if (layer.kind === 'divider') {
    layer.x = clamp(layer.x + dx, 0, 480);
    layer.y = clamp(layer.y + dy, 0, 480);
  }
}

/** Resize text as font/box scaling; images proportionally; dividers along each axis. */
export function resizeLayer(layer: Layer, bounds: LayerBounds, dx: number, dy: number): void {
  if (layer.kind === 'divider') {
    layer.length = clamp(layer.length + (layer.axis === 'horizontal' ? dx : dy), 0.25, 480);
    layer.thickness = clamp(layer.thickness + (layer.axis === 'horizontal' ? dy : dx), 0.25, 16);
    return;
  }
  const change = Math.abs(dx) / Math.max(bounds.width, 1) >= Math.abs(dy) / Math.max(bounds.height, 1)
    ? dx / Math.max(bounds.width, 1) : dy / Math.max(bounds.height, 1);
  let factor = Math.max(0.01, 1 + change);
  if (layer.kind === 'image') {
    factor = clamp(factor, Math.max(1 / layer.width, 1 / layer.height), Math.min(480 / layer.width, 480 / layer.height));
    layer.width = integer(layer.width * factor, 1, 480);
    layer.height = integer(layer.height * factor, 1, 480);
    return;
  }
  const styles = textStyles(layer);
  if (!('style' in layer)) return;
  const origin = layer.style;
  const anchorX = origin.x, anchorY = origin.y;
  let low = 0.01, high = Infinity;
  for (const style of styles) {
    low = Math.max(low, 8 / style.size);
    high = Math.min(high, 160 / style.size);
    if (style.max_width) { low = Math.max(low, 1 / style.max_width); high = Math.min(high, 480 / style.max_width); }
    if (style.line_height) { low = Math.max(low, 8 / style.line_height); high = Math.min(high, 320 / style.line_height); }
    for (const [value, anchor] of [[style.x, anchorX], [style.y, anchorY]]) {
      if (value > anchor) high = Math.min(high, (479 - anchor) / (value - anchor));
      if (value < anchor) high = Math.min(high, anchor / (anchor - value));
    }
  }
  factor = clamp(factor, low, high);
  for (const style of styles) {
    style.x = integer(anchorX + (style.x - anchorX) * factor, 0, 479);
    style.y = integer(anchorY + (style.y - anchorY) * factor, 0, 479);
    style.size = integer(style.size * factor, 8, 160);
    if (style.max_width) style.max_width = integer(style.max_width * factor, 1, 480);
    if (style.line_height) style.line_height = integer(style.line_height * factor, 8, 320);
  }
}

/** Match the renderer's transform anchor and the model's clamping during a gesture. */
export function layerTransform(original:Layer, bounds:LayerBounds, mode:'move'|'resize', dx:number, dy:number) {
  const copy=structuredClone(original);
  const anchor=(layer:Layer):[number,number]=>{
    if('style' in layer)return [layer.style.x,layer.style.y];
    if(layer.kind==='image'&&layer.watermark_position)return [layer.watermark_position.horizontal===0?22:Math.round(layer.watermark_position.horizontal*4.8),layer.watermark_position.vertical===0?22:Math.round(layer.watermark_position.vertical*4.8)];
    if(layer.kind==='image'||layer.kind==='divider')return [layer.x,layer.y];
    return [bounds.x,bounds.y];
  };
  const [ox,oy]=anchor(original);
  if(mode==='move'){
    translateLayer(copy,dx,dy);const [x,y]=anchor(copy);
    return {dx:x-ox,dy:y-oy,sx:1,sy:1,ox,oy};
  }
  resizeLayer(copy,bounds,dx,dy);
  let sx=1,sy=1;
  if('style' in copy&&'style' in original)sx=sy=copy.style.size/original.style.size;
  else if(copy.kind==='image'&&original.kind==='image'){sx=copy.width/original.width;sy=copy.height/original.height;}
  else if(copy.kind==='divider'&&original.kind==='divider'){
    sx=copy.axis==='horizontal'?copy.length/original.length:copy.thickness/original.thickness;
    sy=copy.axis==='horizontal'?copy.thickness/original.thickness:copy.length/original.length;
  }
  return {dx:0,dy:0,sx,sy,ox,oy};
}
