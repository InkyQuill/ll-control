// Deterministic browser font stack using bundled bytes; no host fonts or hardware.
import { chromium } from '@playwright/test';
import { readFileSync } from 'node:fs';
const fonts = ['AbrilFatface-Regular.ttf', 'DejaVuSans.ttf', 'NotoSansTC-Regular.otf'];
const data = fonts.map((file) =>
  readFileSync(new URL(`../../crates/llcontrol-image/assets/${file}`, import.meta.url)).toString(
    'base64',
  ),
);
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const result = await page.evaluate(async (data) => {
    for (let i = 0; i < data.length; i++) {
      const font = new FontFace(`Fallback${i}`, `url(data:font/otf;base64,${data[i]})`);
      await font.load();
      document.fonts.add(font);
    }
    const context = document.createElement('canvas').getContext('2d');
    context.font = '40px Fallback0, Fallback1, Fallback2';
    context.textRendering = 'geometricPrecision';
    context.direction = 'ltr';
    const cases = [
      'Привет',
      '溫度',
      'CPU 溫度 42°C',
      'CPU سلام 42°C',
      'é',
      'e\u0301',
      'Ж\u0301',
      'אבג',
      'سلام',
      'س\u00adلام',
      'س\u2060لام',
    ];
    const widths = Object.fromEntries(cases.map((text) => [text, context.measureText(text).width]));
    context.font = '40px Fallback1';
    const direct_sans_controls = Object.fromEntries(
      ['سلام', 'س\u00adلام', 'س\u2060لام'].map((text) => [text, context.measureText(text).width]),
    );
    return { user_agent: navigator.userAgent, size_px: 40, widths, direct_sans_controls };
  }, data);
  console.log(JSON.stringify({ fonts, ...result }, null, 2));
} finally {
  await browser.close();
}
