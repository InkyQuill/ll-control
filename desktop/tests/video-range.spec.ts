import {test,expect} from '@playwright/test';
import {videoEditBounds,validVideoRange,type SourceMetadata} from '../src/lib/video-range';
const source=(n=30000,d=1001,duration=200):SourceMetadata=>({width:480,height:480,durationSeconds:duration,averageFrameRate:{numerator:n,denominator:d},nominalFrameRate:null});
test('continuous rational range preserves sub-frame values and exceeds VideoCut cap',()=>{
  const bounds=videoEditBounds(source())!;
  expect(bounds.maximumSeconds).toBe(200-1001/30000);
  expect(validVideoRange(0,199,bounds)).toBe(true);
  expect(validVideoRange(1.101001,1.600001,bounds)).toBe(true);
  expect(validVideoRange(0,1001/15000-0.000001,bounds)).toBe(false);
  expect(validVideoRange(0,bounds.maximumSeconds+0.000001,bounds)).toBe(false);
  expect(validVideoRange(NaN,1,bounds)).toBe(false);
});
test('missing/invalid source timing never substitutes output FPS',()=>{
  expect(videoEditBounds(null)).toBeNull();
  expect(videoEditBounds({...source(),averageFrameRate:null})).toBeNull();
  expect(videoEditBounds({...source(),averageFrameRate:null,nominalFrameRate:{numerator:24,denominator:1}})?.fps).toBe(24);
  for(const s of [source(0),source(24,0),source(24,1,0.05),source(24,1,3601),source(24,1,NaN)])expect(videoEditBounds(s)).toBeNull();
});
