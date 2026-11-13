// Layout oracle: exact local font, CSS matching the active Windows text wrapper.
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
      const cases = [
        ['ab\u00adcd', 300],
        ['ab\u00adcd', 70],
        ['ab\u00adcd', 60],
        ['ab\u00adcd', 50],
        ['ab\u00adcd', 40],
        ['a b\u00adcdef', 80],
        ['a b\u00adcdef', 70],
        ['a ab\u00adi', 100],
        ['ab\u00ad', 300],
        ['ab\u00ad\ncd', 300],
      ];
      return {
        user_agent: navigator.userAgent,
        cases: cases.map(([text, width]) => {
          const element = document.createElement('div');
          element.style.cssText = `font:40px/1 ParityDejaVu;white-space:pre-wrap;width:${width}px;text-rendering:geometricPrecision;direction:ltr`;
          element.textContent = text;
          document.body.append(element);
          const groups = [];
          let offset = 0;
          for (const char of text) {
            const range = document.createRange();
            range.setStart(element.firstChild, offset);
            offset += char.length;
            range.setEnd(element.firstChild, offset);
            // A range immediately after SHY can include the previous line's painted hyphen.
            const rects = [...range.getClientRects()];
            const top = rects.at(-1).top;
            const last = groups.at(-1);
            if (last && last.top === top) last.text += char;
            else groups.push({ top, text: char });
          }
          element.remove();
          return { text, width, lines: groups.map((g) => g.text) };
        }),
      };
    },
    readFileSync(
      new URL('../../crates/llcontrol-image/assets/DejaVuSans.ttf', import.meta.url),
    ).toString('base64'),
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
}
