import { addControls } from './editor-controls';
import { test, expect, type Page } from '@playwright/test';
async function setup(page: Page, language: 'en-US' | 'ru-RU' = 'ru-RU', offline = false) {
  await page.addInitScript(
    ({ language, offline }) => {
      localStorage.setItem('llcontrol.interface-language', language);
      const state = {
        version: 1,
        revision: 0,
        temperature_unit: 'celsius',
        interface_language: language,
      };
      const channel = {
        requested: null,
        applied_percent: 70,
        motherboard: false,
        fallback_active: false,
        last_error: null,
      };
      const f = {
        nativeLanguages: [] as string[],
        nativeApplied: '',
        nativeActive: 0,
        nativeMaxActive: 0,
        stallLanguage: '',
        nativeRelease: null as null | (() => void),
        state,
        calls: [] as { method: string; params: Record<string, unknown> }[],
        offline,
      };
      Object.assign(window, {
        languageFixture: f,
        __TAURI_INTERNALS__: {
          invoke: async (command: string, args: Record<string, unknown> = {}) => {
            if (command === 'desktop_set_language') {
              f.nativeLanguages.push(args.language as string);
              f.nativeActive++;
              f.nativeMaxActive = Math.max(f.nativeMaxActive, f.nativeActive);
              if (args.language === f.stallLanguage)
                await new Promise<void>((resolve) => (f.nativeRelease = resolve));
              f.nativeApplied = args.language as string;
              f.nativeActive--;
              return null;
            }
            if (command === 'group_archive_status') return null;
            const method = args.method as string,
              params = (args.params ?? {}) as Record<string, unknown>;
            f.calls.push({ method, params: structuredClone(params) });
            if (f.offline) throw new Error('offline');
            switch (method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'get_display_settings':
                return structuredClone(state);
              case 'set_display_settings': {
                const next = params.settings as typeof state;
                if (next.revision !== state.revision) throw { message: 'stale revision' };
                state.temperature_unit = next.temperature_unit;
                state.interface_language = next.interface_language ?? 'ru-RU';
                state.revision++;
                return structuredClone(state);
              }
              case 'list_devices':
                return [{ id: 'test', display_name: 'Моя помпа', family: 'galahad2-lcdv2' }];
              case 'list_sensors':
                return [
                  {
                    id: 'cpu',
                    label: 'Датчики',
                    unit: 'celsius',
                    value: 40,
                    status: 'valid',
                    age_ms: 0,
                  },
                ];
              case 'get_lcd_state':
                return {
                  source: 'Idle',
                  requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 },
                  last_error: null,
                };
              case 'list_prepared_clips':
              case 'list_theme_assets':
              case 'list_named_theme_assets':
                return [];
              case 'list_profiles':
                return { version: 1, revision: 0, profiles: [] };
              case 'get_cooling_state':
                return { pump: channel, fan: channel };
              case 'get_sensor_defaults':
                return {
                  config: { version: 1, revision: 0, cpu: {}, gpu: null },
                  recommended: { version: 1, revision: 0, cpu: {}, gpu: null },
                  values: [],
                  candidates: [],
                };
              default:
                return null;
            }
          },
        },
      });
    },
    { language, offline },
  );
  await page.goto('/?view=display');
  if (!offline)
    await expect(
      page.getByText(language === 'ru-RU' ? 'Daemon подключён' : 'Daemon connected', {
        exact: true,
      }),
    ).toBeVisible({ timeout: 15000 });
}
test('saving interface language translates navigation and accessible labels while preserving user names and LCD draft', async ({
  page,
}) => {
  await setup(page);
  await page.getByLabel('Название', { exact: true }).fill('Настройки');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByRole('combobox', { name: 'Язык интерфейса', exact: true }).selectOption('en-US');
  await expect(page.getByRole('button', { name: 'Датчики', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Сохранить параметры отображения', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page.getByRole('button', { name: 'Sensors', exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Interface language', exact: true })).toHaveValue(
    'en-US',
  );
  await page.getByRole('button', { name: 'Sensors', exact: true }).click();
  await expect(
    page
      .getByRole('region', { name: 'Sensor readings' })
      .getByRole('heading', { name: 'Датчики', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Pump screen', exact: true }).click();
  await expect(page.getByLabel('Name', { exact: true })).toHaveValue('Настройки');
  await expect(page.getByRole('combobox', { name: 'Device', exact: true })).toHaveValue('test');
  expect(
    await page.evaluate(() =>
      (
        window as unknown as { languageFixture: { calls: { method: string; params: unknown }[] } }
      ).languageFixture.calls.filter((c) => c.method === 'set_display_settings'),
    ),
  ).toEqual([
    {
      method: 'set_display_settings',
      params: {
        settings: {
          version: 1,
          revision: 0,
          temperature_unit: 'celsius',
          interface_language: 'en-US',
        },
      },
    },
  ]);
});
test('external language changes update mounted cooling fields without replacing their unsaved values or issuing control commands', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Охлаждение', exact: true }).click();
  await page.getByRole('spinbutton', { name: 'PWM, %', exact: true }).fill('73');
  await page.evaluate(() => {
    const f = (
      window as unknown as {
        languageFixture: { state: { revision: number; interface_language: string } };
      }
    ).languageFixture;
    f.state.interface_language = 'en-US';
    f.state.revision++;
  });
  await expect(page.getByRole('heading', { name: 'Daemon state · pump' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Apply cooling', exact: true })).toBeVisible();
  await expect(page.getByRole('spinbutton', { name: 'PWM, %', exact: true })).toHaveValue('73');
  await page.evaluate(() => {
    const f = (
      window as unknown as {
        languageFixture: { state: { revision: number; interface_language: string } };
      }
    ).languageFixture;
    f.state.interface_language = 'ru-RU';
    f.state.revision++;
  });
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru');
  await expect(
    page.getByRole('button', { name: 'Применить охлаждение', exact: true }),
  ).toBeVisible();
  await expect(page.getByRole('spinbutton', { name: 'PWM, %', exact: true })).toHaveValue('73');
  expect(
    await page.evaluate(() =>
      (
        window as unknown as { languageFixture: { calls: { method: string }[] } }
      ).languageFixture.calls.filter((c) => c.method.startsWith('set_')),
    ),
  ).toEqual([]);
});
test('confirmed language cache keeps offline navigation accessible without translating a user draft', async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem('llcontrol.interface-language', 'en-US');
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        invoke: async (command: string) => {
          if (command === 'desktop_set_language') return null;
          if (command === 'group_archive_status') return null;
          throw { message: 'Fixture daemon offline' };
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeVisible();
  await expect(page.getByRole('alert').filter({ hasText: 'Fixture daemon offline' })).toBeVisible();
  await page.getByLabel('Name', { exact: true }).fill('Настройки');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByText('Connect to the daemon to load and save display preferences.', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Pump screen', exact: true }).click();
  await expect(page.getByLabel('Name', { exact: true })).toHaveValue('Настройки');
  expect(await page.evaluate(() => localStorage.getItem('llcontrol.interface-language'))).toBe(
    'en-US',
  );
});
for (const cache of ['missing', 'invalid', 'denied', 'ru-RU'] as const) {
  test(`offline default language: ${cache} cache preserves English fallback or explicit Russian`, async ({
    page,
  }) => {
    await page.addInitScript((cache) => {
      localStorage.removeItem('llcontrol.interface-language');
      if (cache === 'ru-RU') localStorage.setItem('llcontrol.interface-language', 'ru-RU');
      if (cache === 'invalid')
        localStorage.setItem('llcontrol.interface-language', 'unknown-locale');
      if (cache === 'denied')
        Object.defineProperty(Storage.prototype, 'getItem', {
          value() {
            throw new Error('Storage denied');
          },
        });
      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: async (command: string) => {
            if (command === 'desktop_set_language') return null;
            if (command === 'group_archive_status') return null;
            throw { message: 'Fixture daemon offline' };
          },
        },
      });
    }, cache);
    await page.goto('/?view=display');
    await expect(page.locator('html')).toHaveAttribute('lang', cache === 'ru-RU' ? 'ru' : 'en');
    await expect(
      page.getByRole('button', { name: cache === 'ru-RU' ? 'Настройки' : 'Settings', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('alert').filter({ hasText: 'Fixture daemon offline' }),
    ).toBeVisible();
  });
}

for (const language of ['en-US', 'ru-RU'] as const) {
  test(`new LCD draft and layout resources use ${language} while existing text survives a language change`, async ({
    page,
  }) => {
    const english = language === 'en-US';
    await setup(page, language);
    await expect(page.getByLabel(english ? 'Name' : 'Название', { exact: true })).toHaveValue(
      english ? 'My screen' : 'Мой экран',
    );
    await page
      .getByRole('button', { name: english ? 'Theme presets' : 'Готовые темы', exact: true })
      .click();
    const layout = page.getByRole('combobox', {
      name: english ? 'Layout' : 'Раскладка',
      exact: true,
    });
    await expect(layout.locator('option[value="focus"]')).toHaveText(
      english ? 'Focus · One large reading and a clock' : 'Focus · Один крупный показатель и часы',
    );
    await expect(layout.locator('option[value="clock"]')).toHaveText(
      english
        ? 'Clock · Date, time and AM/PM with separate colors'
        : 'Clock · Дата, время и AM/PM с отдельными цветами',
    );
    await page
      .getByRole('button', { name: english ? 'Composition' : 'Композиция', exact: true })
      .click();
    await addControls(page);
    await page.getByRole('button', { name: english ? '+ Text' : '+ Текст', exact: true }).click();
    const existing = page.getByLabel(english ? 'Text' : 'Текст', { exact: true });
    await expect(existing.last()).toHaveValue(english ? 'New text' : 'Новый текст');
    await existing.last().fill('Новый текст');
    await page.getByLabel(english ? 'Name' : 'Название', { exact: true }).fill('Мой экран');
    await page
      .getByRole('button', { name: english ? 'Settings' : 'Настройки', exact: true })
      .click();
    await page
      .getByRole('combobox', {
        name: english ? 'Interface language' : 'Язык интерфейса',
        exact: true,
      })
      .selectOption(english ? 'ru-RU' : 'en-US');
    await page
      .getByRole('button', {
        name: english ? 'Save display preferences' : 'Сохранить параметры отображения',
        exact: true,
      })
      .click();
    await expect(page.locator('html')).toHaveAttribute('lang', english ? 'ru' : 'en');
    await page
      .getByRole('button', { name: english ? 'Экран помпы' : 'Pump screen', exact: true })
      .click();
    await expect(page.getByLabel(english ? 'Название' : 'Name', { exact: true })).toHaveValue(
      'Мой экран',
    );
    await expect(page.getByLabel(english ? 'Текст' : 'Text', { exact: true }).last()).toHaveValue(
      'Новый текст',
    );
    await addControls(page);
    await page.getByRole('button', { name: english ? '+ Текст' : '+ Text', exact: true }).click();
    await expect(page.getByLabel(english ? 'Текст' : 'Text', { exact: true }).last()).toHaveValue(
      english ? 'Новый текст' : 'New text',
    );
    const calls = await page.evaluate(
      () =>
        (window as unknown as { languageFixture: { calls: { method: string }[] } }).languageFixture
          .calls,
    );
    expect(
      calls.filter(
        (call) => call.method?.startsWith('set_') && call.method !== 'set_display_settings',
      ),
    ).toEqual([]);
  });
}

for (const language of ['en-US', 'ru-RU'] as const)
  test(`cached ${language} synchronizes native menu without daemon`, async ({ page }) => {
    await setup(page, language, true);
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            (window as unknown as { languageFixture: { nativeLanguages: string[] } })
              .languageFixture.nativeLanguages,
        ),
      )
      .toEqual([language]);
    await expect(page.locator('html')).toHaveAttribute('lang', language === 'en-US' ? 'en' : 'ru');
    const calls = await page.evaluate(
      () =>
        (window as unknown as { languageFixture: { calls: { method: string }[] } }).languageFixture
          .calls,
    );
    expect(calls.some((call) => call.method?.startsWith('set_'))).toBe(false);
  });
test('native language updates serialize and settle on the latest confirmed language', async ({
  page,
}) => {
  await setup(page, 'en-US');
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { languageFixture: { nativeLanguages: string[] } }).languageFixture
            .nativeLanguages,
      ),
    )
    .toEqual(['en-US']);
  await page.evaluate(() => {
    const f = (
      window as unknown as {
        languageFixture: {
          stallLanguage: string;
          state: { interface_language: string; revision: number };
        };
      }
    ).languageFixture;
    f.stallLanguage = 'ru-RU';
    f.state.interface_language = 'ru-RU';
    f.state.revision++;
  });
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru');
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { languageFixture: { nativeLanguages: string[] } }).languageFixture
            .nativeLanguages,
      ),
    )
    .toEqual(['en-US', 'ru-RU']);
  await page.evaluate(() => {
    const f = (
      window as unknown as {
        languageFixture: { state: { interface_language: string; revision: number } };
      }
    ).languageFixture;
    f.state.interface_language = 'en-US';
    f.state.revision++;
  });
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { languageFixture: { nativeLanguages: string[] } }).languageFixture
          .nativeLanguages,
    ),
  ).toEqual(['en-US', 'ru-RU']);
  await page.evaluate(() => {
    (
      window as unknown as { languageFixture: { nativeRelease: (() => void) | null } }
    ).languageFixture.nativeRelease?.();
  });
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { languageFixture: { nativeApplied: string } }).languageFixture
            .nativeApplied,
      ),
    )
    .toBe('en-US');
  const f = await page.evaluate(
    () =>
      (
        window as unknown as {
          languageFixture: {
            nativeMaxActive: number;
            nativeLanguages: string[];
            calls: { method: string }[];
          };
        }
      ).languageFixture,
  );
  expect(f.nativeMaxActive).toBe(1);
  expect(f.nativeLanguages).toEqual(['en-US', 'ru-RU', 'en-US']);
  expect(f.calls.some((call) => call.method?.startsWith('set_'))).toBe(false);
});
