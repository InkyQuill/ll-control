import { test, expect, type Page } from '@playwright/test';
interface State {
  version: 1;
  revision: number;
  enabled: boolean;
  log_path: string | null;
}
interface Fixture {
  state: State;
  language: string;
  offline: boolean;
  lost: boolean;
  hold: boolean;
  calls: { method: string; params: Record<string, unknown> }[];
  pending: (() => void)[];
}
async function setup(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('llcontrol.interface-language', 'en-US');
    const f: Fixture = {
      state: { version: 1, revision: 0, enabled: false, log_path: null },
      language: 'en-US',
      offline: false,
      lost: false,
      hold: false,
      calls: [],
      pending: [],
    };
    Object.assign(window, {
      fpsFixture: f,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          if (command === 'desktop_set_language') return null;
          if (command !== 'daemon_call') return null;
          const method = String(args.method),
            params = (args.params ?? {}) as Record<string, unknown>;
          f.calls.push({ method, params: structuredClone(params) });
          if (f.offline) throw { message: 'Fixture offline' };
          switch (method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
            case 'list_sensors':
            case 'list_prepared_clips':
            case 'list_theme_assets':
            case 'list_named_theme_assets':
              return [];
            case 'list_profiles':
              return { version: 1, revision: 0, profiles: [] };
            case 'get_display_settings':
              return {
                version: 1,
                revision: 0,
                temperature_unit: 'celsius',
                interface_language: f.language,
              };
            case 'get_fps_source':
              return structuredClone(f.state);
            case 'set_fps_source': {
              const next = params.settings as State;
              if (next.revision !== f.state.revision) throw { message: 'stale FPS revision' };
              f.state = { ...next, revision: next.revision + 1 };
              if (f.hold) await new Promise<void>((resolve) => f.pending.push(resolve));
              if (f.lost) {
                f.lost = false;
                throw { message: 'Lost FPS reply' };
              }
              return structuredClone(f.state);
            }
            default:
              return null;
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon connected', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(panel(page).getByLabel('Read FPS', { exact: true })).toBeVisible();
  await expect(panel(page).locator('[data-feature="fps"]')).toContainText('Experimental');
}
const panel = (page: Page) => page.getByRole('region', { name: 'FPS source', exact: true });
const writes = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { fpsFixture: Fixture }).fpsFixture.calls.filter(
      (c) => c.method === 'set_fps_source',
    ),
  );
test('EN/RU global FPS draft requires explicit Save and disabling retains the log without device lookup', async ({
  page,
}) => {
  await setup(page);
  const p = panel(page);
  await p.getByLabel('Read FPS', { exact: true }).check();
  await expect(p.getByRole('button', { name: 'Save FPS settings', exact: true })).toBeDisabled();
  await p.getByLabel('MangoHud log file', { exact: true }).fill('/tmp/Настройки game.csv');
  await page.waitForTimeout(1700);
  expect(await writes(page)).toEqual([]);
  await p.getByRole('button', { name: 'Save FPS settings', exact: true }).click();
  await expect(p.getByRole('status')).toHaveText('FPS settings saved.');
  expect((await writes(page))[0].params).toEqual({
    settings: { version: 1, revision: 0, enabled: true, log_path: '/tmp/Настройки game.csv' },
  });
  await p.getByLabel('Read FPS', { exact: true }).uncheck();
  await p.getByRole('button', { name: 'Save FPS settings', exact: true }).click();
  await expect(p.getByLabel('MangoHud log file', { exact: true })).toHaveValue(
    '/tmp/Настройки game.csv',
  );
  await page.evaluate(() => {
    (window as unknown as { fpsFixture: Fixture }).fpsFixture.language = 'ru-RU';
  });
  const ru = page.getByRole('region', { name: 'Источник FPS', exact: true });
  await expect(ru.getByLabel('Читать FPS', { exact: true })).toBeVisible();
  await expect(ru.locator('[data-feature="fps"]')).toContainText('Экспериментально');
  await expect(ru.getByLabel('Файл лога MangoHud', { exact: true })).toHaveValue(
    '/tmp/Настройки game.csv',
  );
  expect(
    await page.evaluate(
      () => (window as unknown as { fpsFixture: Fixture }).fpsFixture.state.enabled,
    ),
  ).toBe(false);
});
test('lost committed reply preserves draft and requires explicit readback; external edits conflict', async ({
  page,
}) => {
  await setup(page);
  const p = panel(page);
  await p.getByLabel('Read FPS', { exact: true }).check();
  await p.getByLabel('MangoHud log file', { exact: true }).fill('/tmp/game.csv');
  await page.evaluate(() => {
    (window as unknown as { fpsFixture: Fixture }).fpsFixture.lost = true;
  });
  await p.getByRole('button', { name: 'Save FPS settings', exact: true }).click();
  await expect(p.getByRole('alert').filter({ hasText: 'FPS save is unconfirmed' })).toBeVisible();
  await expect(p.getByRole('button', { name: 'Save FPS settings', exact: true })).toBeDisabled();
  await expect(p.getByRole('button', { name: 'Load FPS settings', exact: true })).toBeEnabled();
  await p.getByRole('button', { name: 'Load FPS settings', exact: true }).click();
  await expect(p.getByRole('button', { name: 'Save FPS settings', exact: true })).toBeEnabled();
  expect((await writes(page)).length).toBe(1);
  await page.evaluate(() => {
    const f = (window as unknown as { fpsFixture: Fixture }).fpsFixture;
    f.state = { ...f.state, revision: f.state.revision + 1, enabled: false };
  });
  await expect(
    p.getByRole('alert').filter({ hasText: 'FPS settings changed in another window' }),
  ).toBeVisible();
  await expect(p.getByRole('button', { name: 'Save FPS settings', exact: true })).toBeDisabled();
  await expect(p.getByLabel('Read FPS', { exact: true })).toBeChecked();
});
test('leaving settings while a save is pending does not apply its late result to a new draft', async ({
  page,
}) => {
  await setup(page);
  let p = panel(page);
  await p.getByLabel('Read FPS', { exact: true }).check();
  await p.getByLabel('MangoHud log file', { exact: true }).fill('/tmp/game.csv');
  await page.evaluate(() => {
    (window as unknown as { fpsFixture: Fixture }).fpsFixture.hold = true;
  });
  await p.getByRole('button', { name: 'Save FPS settings', exact: true }).click();
  await page.getByRole('button', { name: 'Sensors', exact: true }).click();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  p = panel(page);
  await expect(p.getByLabel('MangoHud log file', { exact: true })).toHaveValue('/tmp/game.csv');
  await p.getByLabel('MangoHud log file', { exact: true }).fill('/tmp/new-draft.csv');
  await page.evaluate(() => {
    const f = (window as unknown as { fpsFixture: Fixture }).fpsFixture;
    f.hold = false;
    for (const release of f.pending.splice(0)) release();
  });
  await page.waitForTimeout(1700);
  await expect(p.getByLabel('MangoHud log file', { exact: true })).toHaveValue(
    '/tmp/new-draft.csv',
  );
  expect((await writes(page)).length).toBe(1);
});
