// Independent browser advances using the exact bundled font; no device access.
import { chromium } from '@playwright/test';
import { readFileSync } from 'node:fs';
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const result = await page.evaluate(
    async (data) => {
      const font = new FontFace('ParityDejaVu', `url(data:font/ttf;base64,${data})`);
      await font.load();
      document.fonts.add(font);
      const context = document.createElement('canvas').getContext('2d');
      context.font = '40px ParityDejaVu';
      context.fontKerning = 'normal';
      context.direction = 'ltr';
      const strings = [
        'ffi',
        'office',
        'é',
        'e\u0301',
        'سلام',
        '(אבג)',
        'CPU אבג 42°C',
        'CPU سلام 42°C',
      ];
      const modes = {};
      for (const mode of ['auto', 'geometricPrecision']) {
        context.textRendering = mode;
        modes[mode] = Object.fromEntries(
          strings.map((text) => [text, context.measureText(text).width]),
        );
      }
      return {
        user_agent: navigator.userAgent,
        font: 'DejaVuSans.ttf',
        size_px: 40,
        direction: 'ltr',
        modes,
      };
    },
    readFileSync(
      new URL('../../crates/lconnect-image/assets/DejaVuSans.ttf', import.meta.url),
    ).toString('base64'),
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
}
