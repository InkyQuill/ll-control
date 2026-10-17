import { test, expect } from '@playwright/test';

for (const shape of ['square', 'round'] as const) {
  test(`${shape} preview follows display geometry, including corner visibility`, async ({ page }) => {
    await page.goto('/');
    await page.evaluate(async ({ modulePath, shape }) => {
      const { mountGeometry } = await import(/* @vite-ignore */ modulePath);
      mountGeometry({ shape, width: 480, height: 480 });
    }, { modulePath: '/src/lib/__fixtures__/lcd-geometry.ts', shape });
    const preview = page.locator('#lcd-geometry-fixture .lcd');
    await expect(preview).toHaveAttribute('data-lcd-shape', shape);
    await expect(preview.locator('img')).toBeVisible();
    await expect(page.locator(`[data-device-screen="${shape}"]`)).toHaveCount(1);
    await preview.locator('img').evaluate((image: HTMLImageElement) => image.decode());
    const clipping = await preview.evaluate(node => {
      const image = node.querySelector('img')!;
      const box = node.getBoundingClientRect();
      const inset = parseFloat(getComputedStyle(node).borderLeftWidth) + 2;
      const points = [[box.left + inset, box.top + inset], [box.right - inset, box.top + inset],
        [box.left + inset, box.bottom - inset], [box.right - inset, box.bottom - inset]];
      return { radius: getComputedStyle(node).borderTopLeftRadius,
        aspect: box.width / box.height,
        corners: points.map(([x, y]) => document.elementFromPoint(x, y) === image),
        center: document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2) === image };
    });
    expect(clipping.aspect).toBeCloseTo(1, 2);
    expect(clipping.center).toBe(true);
    expect(clipping.corners).toEqual(Array(4).fill(shape === 'square'));
    expect(clipping.radius).toBe(shape === 'square' ? '0px' : '50%');
    await page.locator('#lcd-geometry-fixture').screenshot({ path: `../.impeccable/review/lcd-geometry-${shape}.png` });
  });
}
