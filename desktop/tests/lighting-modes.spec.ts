import { test, expect, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
type Kind = 'fan' | 'pump';
interface Row {
  kind: Kind;
  mode: string;
  colors: number[][];
  speed: number | null;
  brightness: number;
  direction: boolean;
}
const rows = (
  JSON.parse(
    readFileSync(
      new URL(
        '../../crates/llcontrol-ipc/tests/fixtures/lighting-2.1.28-controls.json',
        import.meta.url,
      ),
      'utf8',
    ),
  ) as { rows: Row[] }
).rows;
const hex = (rgb: number[]) => '#' + rgb.map((c) => c.toString(16).padStart(2, '0')).join('');
const mode = (page: Page) => page.getByRole('combobox', { name: 'Эффект', exact: true });
const color = (page: Page, index = 1) =>
  page.getByLabel(index === 1 ? 'Цвет RGB' : `Цвет RGB ${index}`, { exact: true });
async function setup(page: Page) {
  await page.addInitScript(
    ({ rows }) => {
      type Rgb = { r: number; g: number; b: number };
      interface Topology {
        sync_to_pump: boolean;
        size: '280' | '360';
        manual_led_count: number;
      }
      interface Config {
        fan_topology?: Topology | null;
        mode: string;
        color: Rgb;
        additional_colors: Rgb[];
        brightness_percent: number;
        speed_percent: number;
        direction: string;
        scope: string | null;
        disabled: boolean;
      }
      const rgb = (c: number[]): Rgb => ({ r: c[0], g: c[1], b: c[2] });
      const catalog = (kind: string) =>
        rows
          .filter((row) => row.kind === kind)
          .map((row) => ({
            mode: row.mode,
            color_count: row.colors.length,
            speed: row.speed !== null,
            brightness: true,
            direction: row.direction,
            defaults: {
              mode: row.mode,
              color: rgb(row.colors[0] ?? [0, 0, 0]),
              additional_colors: row.colors.slice(1).map(rgb),
              brightness_percent: row.brightness,
              speed_percent: row.speed ?? 0,
              direction: 'forward',
              scope: kind === 'pump' ? 'inner' : null,
              disabled: false,
            },
          }));
      const saved = JSON.parse(
        localStorage.getItem('mode-fixture') ?? '{"base":{},"profiles":{},"writes":0}',
      ) as {
        topologies?: Record<string, Topology>;
        base: Record<string, Config>;
        profiles: Record<string, Record<string, Config>>;
        writes: number;
      };
      saved.topologies ??= {};
      Object.assign(window, {
        modeFixture: saved,
        __TAURI_INTERNALS__: {
          invoke: async (
            _: string,
            req: { method: string; params: { dev: string; kind: string; cfg: Config } },
          ) => {
            if (_ === 'desktop_set_language') return null;
            const key = req.params ? `${req.params.dev}:${req.params.kind}` : '';
            switch (req.method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'list_devices':
                return [
                  { id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' },
                  { id: 'B', display_name: 'LCD B', family: 'galahad2-lcdv2' },
                ];
              case 'list_sensors':
                return [];
              case 'get_lcd_state':
                return { source: 'Idle', requested: {} };
              case 'get_lighting_info':
                return {
                  fan_topology:
                    req.params.kind === 'fan'
                      ? (saved.topologies![req.params.dev] ?? {
                          sync_to_pump: false,
                          size: '360',
                          manual_led_count: 24,
                        })
                      : null,
                  modes: catalog(req.params.kind).map((c) => c.mode),
                  catalog: catalog(req.params.kind),
                  base: saved.base[key] ?? null,
                  profiles: Object.values(saved.profiles[key] ?? {}),
                };
              case 'get_thermal_state':
                return {
                  requested: { cpu: null, gpu: null },
                  degraded_sources: [],
                  fan: { last_error: null },
                  pump: { last_error: null },
                  grace_remaining_ms: 0,
                };
              case 'set_lighting':
                if (req.params.kind === 'fan' && req.params.cfg.fan_topology)
                  saved.topologies![req.params.dev] = structuredClone(req.params.cfg.fan_topology);
                saved.writes++;
                saved.base[key] = structuredClone(req.params.cfg);
                (saved.profiles[key] ??= {})[req.params.cfg.mode] = structuredClone(req.params.cfg);
                localStorage.setItem('mode-fixture', JSON.stringify(saved));
                return null;
              default:
                return null;
            }
          },
        },
      });
    },
    { rows },
  );
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await expect(mode(page)).toHaveValue('rainbow');
}
test('all Windows mode controls and defaults appear from the daemon catalog', async ({ page }) => {
  await setup(page);
  for (const kind of ['pump', 'fan'] as const) {
    await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption(kind);
    for (const row of rows.filter((r) => r.kind === kind)) {
      await mode(page).selectOption(row.mode);
      await expect(page.getByRole('slider', { name: /Скорость:/ })).toHaveCount(
        row.speed === null ? 0 : 1,
      );
      await expect(page.getByRole('combobox', { name: 'Направление', exact: true })).toHaveCount(
        row.direction ? 1 : 0,
      );
      for (let index = 0; index < 4; index++) {
        if (index < row.colors.length)
          await expect(color(page, index + 1)).toHaveValue(hex(row.colors[index]));
        else await expect(color(page, index + 1)).toHaveCount(0);
      }
      await expect(page.getByRole('slider', { name: /Яркость RGB:/ })).toHaveValue('100');
      if (row.speed !== null)
        await expect(page.getByRole('slider', { name: /Скорость:/ })).toHaveValue(
          String(row.speed),
        );
    }
  }
  expect(
    await page.evaluate(
      () => (window as unknown as { modeFixture: { writes: number } }).modeFixture.writes,
    ),
  ).toBe(0);
});
test('unapplied mode drafts are independent across modes zones and devices', async ({ page }) => {
  await setup(page);
  await mode(page).selectOption('meteor');
  await color(page, 4).fill('#112233');
  await page.getByRole('slider', { name: /Скорость:/ }).fill('100');
  await mode(page).selectOption('runway');
  await color(page).fill('#445566');
  await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption('fan');
  await mode(page).selectOption('meteor');
  await expect(color(page)).toHaveValue('#00d7ff');
  await color(page).fill('#778899');
  await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption('pump');
  await expect(mode(page)).toHaveValue('runway');
  await expect(color(page)).toHaveValue('#445566');
  await mode(page).selectOption('meteor');
  await expect(color(page, 4)).toHaveValue('#112233');
  await expect(page.getByRole('slider', { name: /Скорость:/ })).toHaveValue('100');
  await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('B');
  await expect(mode(page)).toHaveValue('rainbow');
  await mode(page).selectOption('meteor');
  await expect(color(page, 4)).toHaveValue('#ffff00');
  await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('A');
  await expect(mode(page)).toHaveValue('meteor');
  await expect(color(page, 4)).toHaveValue('#112233');
});
test('reset mode defaults changes only the draft and preserves other modes', async ({ page }) => {
  await setup(page);
  await mode(page).selectOption('runway');
  await color(page).fill('#112233');
  await mode(page).selectOption('meteor');
  await color(page, 4).fill('#445566');
  await page.getByRole('button', { name: 'Значения режима по умолчанию', exact: true }).click();
  await expect(color(page, 4)).toHaveValue('#ffff00');
  await mode(page).selectOption('runway');
  await expect(color(page)).toHaveValue('#112233');
  expect(
    await page.evaluate(
      () => (window as unknown as { modeFixture: { writes: number } }).modeFixture.writes,
    ),
  ).toBe(0);
});
test('applied per-mode settings restore after GUI reload without implicit Apply', async ({
  page,
}) => {
  await setup(page);
  await mode(page).selectOption('runway');
  await color(page).fill('#112233');
  await page.getByRole('button', { name: 'Применить подсветку', exact: true }).click();
  await mode(page).selectOption('meteor');
  await color(page, 4).fill('#445566');
  await page.getByRole('button', { name: 'Применить подсветку', exact: true }).click();
  await page.reload();
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await expect(mode(page)).toHaveValue('meteor');
  await expect(color(page, 4)).toHaveValue('#445566');
  await mode(page).selectOption('runway');
  await expect(color(page)).toHaveValue('#112233');
  expect(
    await page.evaluate(
      () => (window as unknown as { modeFixture: { writes: number } }).modeFixture.writes,
    ),
  ).toBe(2);
});

test('fan topology is global across modes and preserves manual count while synced', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption('fan');
  const count = page.getByRole('spinbutton', { name: 'Число LED в ручном режиме', exact: true });
  await count.fill('31');
  await page
    .getByRole('checkbox', { name: 'Синхронизировать вентиляторы с помпой', exact: true })
    .check();
  await page
    .getByRole('combobox', { name: 'Размер комплекта вентиляторов', exact: true })
    .selectOption('280');
  await mode(page).selectOption('meteor');
  await page.getByRole('button', { name: 'Значения режима по умолчанию', exact: true }).click();
  await expect(count).toHaveValue('31');
  await expect(
    page.getByRole('combobox', { name: 'Размер комплекта вентиляторов', exact: true }),
  ).toHaveValue('280');
  await page.getByRole('button', { name: 'Применить подсветку', exact: true }).click();
  await page
    .getByRole('checkbox', { name: 'Синхронизировать вентиляторы с помпой', exact: true })
    .uncheck();
  await count.fill('8');
  await page.getByRole('button', { name: 'Загрузить настройки подсветки', exact: true }).click();
  await expect(count).toHaveValue('31');
  await expect(
    page.getByRole('checkbox', { name: 'Синхронизировать вентиляторы с помпой', exact: true }),
  ).toBeChecked();
  await mode(page).selectOption('runway');
  await expect(count).toHaveValue('31');
  await page
    .getByRole('checkbox', { name: 'Синхронизировать вентиляторы с помпой', exact: true })
    .uncheck();
  await expect(count).toHaveValue('31');
  await page.reload();
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption('fan');
  await expect(count).toHaveValue('31');
  await expect(
    page.getByRole('checkbox', { name: 'Синхронизировать вентиляторы с помпой', exact: true }),
  ).toBeChecked();
});
test('fan LED bounds block invalid Apply and topology drafts stay with their device', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption('fan');
  const count = page.getByRole('spinbutton', { name: 'Число LED в ручном режиме', exact: true });
  const apply = page.getByRole('button', { name: 'Применить подсветку', exact: true });
  for (const value of ['7', '51', '8.5', '']) {
    await count.fill(value);
    await expect(apply).toBeDisabled();
  }
  for (const value of ['8', '50']) {
    await count.fill(value);
    await expect(apply).toBeEnabled();
  }
  await count.fill('31');
  await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('B');
  await expect(count).toHaveValue('24');
  await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('A');
  await expect(count).toHaveValue('31');
  expect(
    await page.evaluate(
      () => (window as unknown as { modeFixture: { writes: number } }).modeFixture.writes,
    ),
  ).toBe(0);
});
