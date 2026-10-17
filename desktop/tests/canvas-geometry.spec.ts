import { expect, test } from '@playwright/test';
import { nativePoint, resizeLayer, translateLayer } from '../src/lib/canvas-geometry';
import type { Layer, Style } from '../src/lib/api';
const style = (x: number, y: number): Style => ({x, y, size: 20, color: [255,255,255], align: 'left'});

test('scaled rotated pointer coordinates return to native canvas space', () => {
  expect(nativePoint(60, 30, 240, 0)).toEqual([120,60]);
  expect(nativePoint(60, 30, 240, 90)).toEqual([60,360]);
  expect(nativePoint(60, 30, 240, 180)).toEqual([360,420]);
  expect(nativePoint(60, 30, 240, 270)).toEqual([420,120]);
});

test('moving sensor parts clamps the group without losing relative positions or imported styles', () => {
  const layer: Layer = {kind:'sensor', sensor_id:'CPU', label:'CPU', unit:'celsius', temperature_unit:'system', decimals:1,
    style: {...style(100,100), font_asset:'managed', line_box:true, shadow:{x:2,y:3,blur:4,color:[0,0,0],opacity:50}},
    parts:{label:style(10,30),unit:style(450,470)}};
  const before = structuredClone(layer);
  translateLayer(layer, 90,90);
  expect(layer.style.x).toBe(129); expect(layer.style.y).toBe(109);
  expect(layer.parts!.label.x).toBe(39); expect(layer.parts!.unit.x).toBe(479);
  expect(layer.parts!.unit.y).toBe(479);
  expect(layer.style.shadow).toEqual(before.style.shadow);
  expect(layer.style.font_asset).toBe('managed');
  translateLayer(layer, -1000,-1000);
  expect(layer.parts!.label.x).toBe(0); expect(layer.parts!.label.y).toBe(0);
  expect(layer.style.x).toBe(90); expect(layer.style.y).toBe(70);
});

test('text resizing scales font, wrapping and parts while keeping the value anchor', () => {
  const layer: Layer = {kind:'sensor_cycle', sensors:[], interval_seconds:2,
    style:{...style(100,100),max_width:100,line_height:24},parts:{label:style(50,50),unit:style(150,150)}};
  resizeLayer(layer,{x:50,y:50,width:150,height:150},75,75);
  expect(layer.style).toMatchObject({x:100,y:100,size:30,max_width:150,line_height:36});
  expect(layer.parts!.label).toMatchObject({x:25,y:25,size:30});
  expect(layer.parts!.unit).toMatchObject({x:175,y:175,size:30});
});

test('image and divider transforms retain fit, opacity and bounded geometry', () => {
  const image: Layer = {kind:'image',asset:'logo',x:400,y:30,width:100,height:50,fit:'contain_native',opacity:80};
  resizeLayer(image,{x:400,y:30,width:100,height:50},100,50);
  translateLayer(image,1000,-1000);
  expect(image).toMatchObject({x:480,y:0,width:200,height:100,fit:'contain_native',opacity:80});
  const divider: Layer = {kind:'divider',axis:'vertical',x:10,y:10,length:100,thickness:2,color:[255,0,0],opacity:100};
  resizeLayer(divider,{x:10,y:10,width:2,height:100},50,-200);
  expect(divider).toMatchObject({length:0.25,thickness:16});
});
