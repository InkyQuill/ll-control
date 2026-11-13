import { test, expect } from '@playwright/test';
import type { Theme } from '../src/lib/api';

interface PreviewFixture {
  online: boolean;
  calls: { method: string; params: { dev?: string; theme?: Theme } }[];
  pending: ((fail: boolean) => void)[];
  active: number;
  max: number;
}
declare global {
  interface Window {
    previewRecovery: PreviewFixture;
  }
}

for (const transition of ['draft', 'device', 'reconnect'] as const) {
  for (const lateError of [false, true]) {
    test(`preview ${transition}: discard late ${lateError ? 'error' : 'image'} and recover automatically`, async ({
      page,
    }) => {
      await page.addInitScript(() => {
        const fixture: PreviewFixture = { online: true, calls: [], pending: [], active: 0, max: 0 };
        window.previewRecovery = fixture;
        Object.assign(window, {
          __TAURI_INTERNALS__: {
            invoke: async (_: string, request: PreviewFixture['calls'][number]) => {
              if (_ === 'desktop_set_language') return null;
              if (!fixture.online) throw new Error('fixture offline');
              fixture.calls.push(request);
              switch (request.method) {
                case 'list_devices':
                  return [
                    { id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' },
                    { id: 'B', display_name: 'LCD B', family: 'galahad2-lcdv2' },
                  ];
                case 'list_sensors':
                  return [];
                case 'get_lcd_state':
                  return { source: 'Idle', last_error: null };
                case 'preview_lcd_editor': {
                  fixture.active++;
                  fixture.max = Math.max(fixture.max, fixture.active);
                  try {
                    await new Promise<void>((resolve, reject) =>
                      fixture.pending.push((fail) =>
                        fail ? reject(new Error('Preview fixture failed')) : resolve(),
                      ),
                    );
                    return Array.from(
                      atob(
                        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII=',
                      ),
                      (c) => c.charCodeAt(0),
                    );
                  } finally {
                    fixture.active--;
                  }
                }
                default:
                  throw new Error('Unexpected ' + request.method);
              }
            },
          },
        });
      });
      await page.goto('/?view=display');
      await expect(page.getByText('Daemon подключён')).toBeVisible();
      const title = page.getByLabel('Название', { exact: true });
      const count = () =>
        page.evaluate(
          () =>
            window.previewRecovery.calls.filter((c) => c.method === 'preview_lcd_editor').length,
        );
      const finish = (fail: boolean) =>
        page.evaluate((fail) => window.previewRecovery.pending.shift()!(fail), fail);
      await title.fill('Before transition');
      await expect.poll(count).toBe(1);
      if (transition === 'draft') await title.fill('After transition');
      if (transition === 'device')
        await page.getByLabel('Устройство', { exact: true }).selectOption('B');
      if (transition === 'reconnect') {
        await page.evaluate(() => {
          window.previewRecovery.online = false;
        });
        await expect(page.getByText('Нет связи с daemon')).toBeVisible();
        await page.evaluate(() => {
          window.previewRecovery.online = true;
        });
        await page.getByRole('button', { name: 'Повторить подключение' }).click();
        await expect(page.getByText('Daemon подключён')).toBeVisible();
      }
      await finish(lateError);
      await expect(page.getByRole('img')).toHaveCount(0);
      await expect(page.getByText('Preview fixture failed', { exact: true })).toHaveCount(0);
      await expect(
        page.getByText('Preview обновлён. На устройство ничего не отправлено.', { exact: true }),
      ).toHaveCount(0);
      await expect(title).toHaveValue(
        transition === 'draft' ? 'After transition' : 'Before transition',
      );
      // A current failure is visible and recoverable, unlike the obsolete reply.
      await expect.poll(count).toBe(2);
      await finish(true);
      await expect(
        page.getByRole('alert').filter({ hasText: 'Preview fixture failed' }),
      ).toBeVisible();
      await expect.poll(count).toBe(3);
      await finish(false);
      await expect(page.getByRole('img')).toBeVisible();
      await expect(page.getByText('Preview fixture failed', { exact: true })).toHaveCount(0);
      const evidence = await page.evaluate(() => ({
        calls: window.previewRecovery.calls,
        max: window.previewRecovery.max,
      }));
      const last = evidence.calls.filter((c) => c.method === 'preview_lcd_editor').at(-1)!;
      expect(last.params.dev).toBe(transition === 'device' ? 'B' : 'A');
      expect(last.params.theme?.name).toBe(
        transition === 'draft' ? 'After transition' : 'Before transition',
      );
      expect(evidence.max).toBe(1);
      expect(
        evidence.calls.filter((c) => c.method.startsWith('set_') || c.method === 'clear_lcd'),
      ).toEqual([]);
    });
  }
}
