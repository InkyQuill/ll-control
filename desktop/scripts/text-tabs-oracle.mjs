// CSS default tab-size:8, measured in actual DOM with the bundled font.
import { chromium } from '@playwright/test';
import { readFileSync } from 'node:fs';
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const result = await page.evaluate(
    async (data) => {
      const face = new FontFace('ParityDejaVu', `url(data:font/ttf;base64,${data})`);
      await face.load();
      document.fonts.add(face);
      const cases = [];
      for (const align of ['left', 'center', 'right']) {
        for (const text of ['A\tB', '\tA', 'A\t\tB', 'אבג\tX', 'A\tאבג', 'AAA \tB', 'A\t\nB']) {
          const el = document.createElement('div');
          el.style.cssText = `font:40px/1 ParityDejaVu;white-space:pre-wrap;width:480px;text-align:${align};text-rendering:geometricPrecision;direction:ltr`;
          el.textContent = text;
          document.body.append(el);
          const box = el.getBoundingClientRect();
          const glyphs = [];
          for (let i = 0; i < text.length; i++) {
            const range = document.createRange();
            range.setStart(el.firstChild, i);
            range.setEnd(el.firstChild, i + 1);
            const r = range.getBoundingClientRect();
            glyphs.push({ char: text[i], x: r.x - box.x, y: r.y - box.y, width: r.width });
          }
          cases.push({ align, text, glyphs });
          el.remove();
        }
      }
      return { user_agent: navigator.userAgent, cases };
    },
    readFileSync(
      new URL('../../crates/lconnect-image/assets/DejaVuSans.ttf', import.meta.url),
    ).toString('base64'),
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
}
