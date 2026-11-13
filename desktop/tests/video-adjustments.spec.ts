import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { videoChannelLut } from '../src/lib/video-adjustments';

test('preview channel table matches all installed SDK oracle vectors', () => {
  const oracle = readFileSync(
    new URL(
      '../../crates/lconnect-media/tests/fixtures/video-effect-channels.bin',
      import.meta.url,
    ),
  );
  for (let amount = 0; amount < 256; amount++) {
    expect(Buffer.from(videoChannelLut({ contrast: amount, brightness: 0 }))).toEqual(
      oracle.subarray(amount * 256, (amount + 1) * 256),
    );
    expect(Buffer.from(videoChannelLut({ contrast: 0, brightness: amount }))).toEqual(
      oracle.subarray(65536 + amount * 256, 65536 + (amount + 1) * 256),
    );
  }
  for (const contrast of [1, 32, 128, 255])
    for (const brightness of [1, 32, 128, 255]) {
      const expected = Uint8Array.from(
        { length: 256 },
        (_, value) => oracle[65536 + brightness * 256 + oracle[contrast * 256 + value]],
      );
      expect(videoChannelLut({ contrast, brightness })).toEqual(expected);
    }
  for (const value of [-1, 256, 1.5, NaN, Infinity])
    expect(() => videoChannelLut({ contrast: value, brightness: 0 })).toThrow();
});
