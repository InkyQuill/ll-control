import { expect, type Page } from '@playwright/test';

export async function inspect(page: Page) {
  const controls = page.locator(
    'main input:visible, main select:visible, main textarea:visible, main button:visible, main summary:visible',
  );
  for (const control of await controls.all()) await expect(control).toHaveAccessibleName(/\S/);
  const failures = await page.evaluate(() => {
    type Color = [number, number, number, number];
    const rgba = (value: string): Color => {
      const c = value.match(/[\d.]+/g)!.map(Number);
      return [c[0], c[1], c[2], c[3] ?? 1];
    };
    const over = (fg: Color, bg: Color): Color => [
      fg[0] * fg[3] + bg[0] * (1 - fg[3]),
      fg[1] * fg[3] + bg[1] * (1 - fg[3]),
      fg[2] * fg[3] + bg[2] * (1 - fg[3]),
      1,
    ];
    const luminance = (c: Color) => {
      const v = c
        .slice(0, 3)
        .map((x) => x / 255)
        .map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
      return v[0] * 0.2126 + v[1] * 0.7152 + v[2] * 0.0722;
    };
    const failures: { text: string; ratio: number; minimum: number }[] = [];
    const walker = document.createTreeWalker(document.querySelector('main')!, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode,
        el = node.parentElement;
      if (
        !el ||
        !node.textContent?.trim() ||
        !el.checkVisibility({ checkVisibilityCSS: true }) ||
        el.closest('option, :disabled')
      )
        continue;
      const ancestors: Element[] = [];
      for (let p: Element | null = el; p; p = p.parentElement) ancestors.unshift(p);
      let background: Color = [255, 255, 255, 1];
      for (const p of ancestors)
        background = over(rgba(getComputedStyle(p).backgroundColor), background);
      const style = getComputedStyle(el),
        foreground = over(rgba(style.color), background);
      const a = luminance(foreground),
        b = luminance(background),
        ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      const size = parseFloat(style.fontSize),
        bold = parseInt(style.fontWeight) >= 700;
      const minimum = size >= 24 || (bold && size >= 18.6667) ? 3 : 4.5;
      if (ratio < minimum)
        failures.push({ text: node.textContent.trim().slice(0, 100), ratio, minimum });
    }
    return failures;
  });
  expect(failures).toEqual([]);
}
