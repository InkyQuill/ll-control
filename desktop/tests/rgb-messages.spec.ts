import { test, expect, type Page } from '@playwright/test';
interface Fixture {
  temperatureUnit: string;
  requests: { method: string; params: Record<string, unknown> }[];
  diagnostic: string;
  language: string;
  fail: string;
  lastError: string | null;
  calls: string[];
}
async function setup(page: Page, language: string, temperatureUnit = 'celsius') {
  await page.addInitScript(
    ({ language, temperatureUnit }) => {
      localStorage.setItem('lconnect.interface-language', language);
      const f: Fixture = {
        temperatureUnit,
        requests: [],
        diagnostic: 'Настройки',
        language,
        fail: '',
        lastError: null,
        calls: [],
      };
      const binding = {
        sensor_id: 'cpu',
        unit: 'celsius',
        low_value: 30,
        high_value: 60,
        low_colors: [{ r: 0, g: 0, b: 255 }],
        high_colors: [{ r: 255, g: 0, b: 0 }],
        low_speed_percent: null,
        high_speed_percent: null,
      };
      const dynamic = () => ({
        base_mode: 'static',
        color_count: 1,
        speed_supported: false,
        requested: binding,
        applied: null,
        degraded: false,
        suppressed_by_thermal: false,
        last_error: f.lastError,
      });
      const thermal = () => ({
        requested: { cpu: null, gpu: null },
        active_source: null,
        degraded_sources: [],
        grace_remaining_ms: 0,
        blocked_reason: null,
        fan: { last_error: null },
        pump: { last_error: null },
      });
      Object.assign(window, {
        rgbMessages: f,
        __TAURI_INTERNALS__: {
          invoke: async (command: string, args: Record<string, unknown> = {}) => {
            if (command !== 'daemon_call') return null;
            const method = String(args.method);
            f.calls.push(method);
            f.requests.push({
              method,
              params: structuredClone((args.params ?? {}) as Record<string, unknown>),
            });
            if (f.fail === method) throw { message: f.diagnostic };
            switch (method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'get_display_settings':
                return {
                  version: 1,
                  revision: 0,
                  temperature_unit: f.temperatureUnit,
                  interface_language: f.language,
                };
              case 'list_devices':
                return [{ id: 'lcd', display_name: 'Fixture LCDV2', family: 'galahad2-lcdv2' }];
              case 'list_sensors':
                return [
                  {
                    id: 'rpm',
                    label: 'Pump RPM',
                    unit: 'rpm',
                    value: 1000,
                    status: 'valid',
                    age_ms: 0,
                  },
                  {
                    id: 'cpu',
                    label: 'Настройки',
                    unit: 'celsius',
                    value: 45,
                    status: 'valid',
                    age_ms: 0,
                  },
                ];
              case 'get_lcd_state':
                return { source: 'Idle', last_error: null, requested: {} };
              case 'get_lighting_info':
                return {
                  modes: ['static'],
                  max_brightness: 100,
                  max_speed: 100,
                  base: {
                    mode: 'static',
                    color: { r: 0, g: 0, b: 255 },
                    brightness_percent: 100,
                    speed_percent: 0,
                    direction: 'forward',
                    scope: 'inner',
                    disabled: false,
                  },
                };
              case 'get_thermal_state':
              case 'set_thermal_config':
                return thermal();
              case 'get_dynamic_rgb':
              case 'set_dynamic_rgb':
                return dynamic();
              case 'list_profiles':
                return { version: 1, revision: 0, profiles: [] };
              case 'list_prepared_clips':
              case 'list_theme_assets':
              case 'list_named_theme_assets':
                return [];
              default:
                return null;
            }
          },
        },
      });
    },
    { language, temperatureUnit },
  );
  await page.goto('/?view=display');
  await page
    .getByRole('button', { name: language === 'en-US' ? 'Lighting' : 'Подсветка', exact: true })
    .click();
  await expect(
    page.getByRole('button', { name: /^(Apply lighting|Применить подсветку)$/ }),
  ).toBeEnabled();
}
const dynamic = (page: Page) =>
  page.locator('details').filter({
    has: page
      .locator('summary')
      .filter({ hasText: /^(Sensor-driven pump lighting|Подсветка помпы по датчику)$/ }),
  });
async function openDynamic(page: Page) {
  await page
    .getByText(/^(Sensor-driven pump lighting|Подсветка помпы по датчику)$/, { exact: true })
    .click();
  await page
    .getByRole('button', {
      name: /^(Load sensor-driven mode settings|Загрузить настройки режима по датчику)$/,
    })
    .click();
}
async function language(page: Page, value: string) {
  await page.evaluate((value) => {
    (window as unknown as { rgbMessages: Fixture }).rgbMessages.language = value;
  }, value);
  await expect(page.locator('html')).toHaveAttribute('lang', value === 'en-US' ? 'en' : 'ru');
}
const writes = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { rgbMessages: Fixture }).rgbMessages.calls.filter((x) =>
      x.startsWith('set_'),
    ),
  );
for (const initial of ['en-US', 'ru-RU']) {
  const en = initial === 'en-US',
    other = en ? 'ru-RU' : 'en-US';
  for (const context of [
    'manual',
    'load',
    'thermal-poll',
    'thermal-apply',
    'dynamic-poll',
    'dynamic-apply',
    'dynamic-output',
  ] as const)
    test(`${context} RGB diagnostic remains raw across language change (${initial})`, async ({
      page,
    }) => {
      await setup(page, initial);
      if (context.startsWith('dynamic')) await openDynamic(page);
      const method = {
        manual: 'set_lighting',
        load: 'get_lighting_info',
        'thermal-poll': 'get_thermal_state',
        'thermal-apply': 'set_thermal_config',
        'dynamic-poll': 'get_dynamic_rgb',
        'dynamic-apply': 'set_dynamic_rgb',
        'dynamic-output': '',
      }[context];
      await page.evaluate(
        ({ method, output }) => {
          const f = (window as unknown as { rgbMessages: Fixture }).rgbMessages;
          f.fail = method;
          if (output) f.lastError = 'Настройки';
        },
        { method, output: context === 'dynamic-output' },
      );
      if (context === 'manual')
        await page.getByRole('button', { name: /^(Apply lighting|Применить подсветку)$/ }).click();
      if (context === 'load')
        await page
          .getByRole('button', { name: /^(Load lighting settings|Загрузить настройки подсветки)$/ })
          .click();
      if (context === 'thermal-apply') {
        await page
          .getByRole('button', { name: /^(Load current alerts|Загрузить текущие оповещения)$/ })
          .click();
        await page.getByRole('button', { name: /^(Apply alerts|Применить оповещения)$/ }).click();
      }
      if (context === 'dynamic-apply')
        await page
          .getByRole('button', { name: /^(Apply sensor-driven mode|Применить режим по датчику)$/ })
          .click();
      const scope = context.startsWith('dynamic')
        ? dynamic(page)
        : page.getByRole('region', { name: /^(Lighting|Подсветка)$/ });
      await expect(
        scope
          .getByRole('alert')
          .filter({ has: page.locator('span').filter({ hasText: /^Настройки$/ }) })
          .first(),
      ).toBeVisible();
      const prefix = (english: boolean) =>
        context === 'dynamic-output'
          ? null
          : context === 'dynamic-poll'
            ? english
              ? 'Could not read sensor-driven lighting:'
              : 'Не удалось прочитать режим подсветки по датчику:'
            : context === 'dynamic-apply'
              ? english
                ? 'Sensor-driven lighting error:'
                : 'Ошибка подсветки по датчику:'
              : context === 'thermal-poll'
                ? english
                  ? 'Could not read thermal alert state:'
                  : 'Не удалось прочитать состояние оповещений:'
                : english
                  ? 'Lighting error:'
                  : 'Ошибка подсветки:';
      if (prefix(en))
        await expect(scope.getByRole('alert').filter({ hasText: prefix(en)! })).toBeVisible();
      await language(page, other);
      await expect(
        scope
          .getByRole('alert')
          .locator('span')
          .filter({ hasText: /^Настройки$/ })
          .first(),
      ).toBeVisible();
      if (prefix(!en))
        await expect(scope.getByRole('alert').filter({ hasText: prefix(!en)! })).toBeVisible();
      expect(await writes(page)).toEqual(
        context === 'manual'
          ? ['set_lighting']
          : context === 'thermal-apply'
            ? ['set_thermal_config']
            : context === 'dynamic-apply'
              ? ['set_dynamic_rgb']
              : [],
      );
    });
  test(`dynamic range validation follows language, preserves draft and sends no write (${initial})`, async ({
    page,
  }) => {
    await setup(page, initial);
    await openDynamic(page);
    const p = dynamic(page);
    await p
      .getByRole('spinbutton', { name: /^(Lower threshold \(°C\)|Нижний порог \(°C\))$/ })
      .fill('70');
    await p
      .getByRole('button', { name: /^(Apply sensor-driven mode|Применить режим по датчику)$/ })
      .click();
    const expected = (english: boolean) =>
      english
        ? 'Temperature thresholds must correspond to whole degrees Celsius from 0 to 100, with the lower value no higher than the upper value.'
        : 'Пороги температуры должны соответствовать целым °C от 0 до 100; нижний не выше верхнего.';
    await expect(p.getByRole('alert')).toHaveText(expected(en));
    await language(page, other);
    await expect(p.getByRole('alert')).toHaveText(expected(!en));
    await expect(
      p.getByRole('spinbutton', { name: /^(Lower threshold \(°C\)|Нижний порог \(°C\))$/ }),
    ).toHaveValue('70');
    expect(await writes(page)).toEqual([]);
  });
}

for (const initial of ['en-US', 'ru-RU'])
  for (const thermal of [false, true])
    test(`thermal restore hint is scoped to alerts and preserves its raw cause (${initial}, ${thermal})`, async ({
      page,
    }) => {
      await setup(page, initial);
      const detail = 'invalid argument: thermal alerts require known fan and pump lighting bases';
      await page.evaluate(
        ({ detail, thermal }) => {
          const f = (window as unknown as { rgbMessages: Fixture }).rgbMessages;
          f.diagnostic = detail;
          f.fail = thermal ? 'set_thermal_config' : 'set_lighting';
        },
        { detail, thermal },
      );
      if (thermal) {
        await page
          .getByRole('button', { name: /^(Load current alerts|Загрузить текущие оповещения)$/ })
          .click();
        await page.getByRole('button', { name: /^(Apply alerts|Применить оповещения)$/ }).click();
      } else
        await page.getByRole('button', { name: /^(Apply lighting|Применить подсветку)$/ }).click();
      const alert = page.getByRole('region', { name: /^(Lighting|Подсветка)$/ }).getByRole('alert');
      await expect(alert.locator('span')).toHaveText(detail);
      const hint = (en: boolean) =>
        en
          ? 'First apply normal lighting to both zones'
          : 'Сначала примените обычную подсветку в обеих зонах';
      if (thermal) await expect(alert).toContainText(hint(initial === 'en-US'));
      else await expect(alert).not.toContainText(hint(initial === 'en-US'));
      await language(page, initial === 'en-US' ? 'ru-RU' : 'en-US');
      await expect(alert.locator('span')).toHaveText(detail);
      if (thermal) await expect(alert).toContainText(hint(initial !== 'en-US'));
      else await expect(alert).not.toContainText(hint(initial !== 'en-US'));
      expect(await writes(page)).toEqual([thermal ? 'set_thermal_config' : 'set_lighting']);
    });
for (const initial of ['en-US', 'ru-RU'])
  test(`RPM validation retains the 3600 range in both languages (${initial})`, async ({ page }) => {
    await setup(page, initial);
    await openDynamic(page);
    const p = dynamic(page);
    await p
      .getByRole('combobox', { name: /^(Lighting sensor|Датчик подсветки)$/ })
      .selectOption('rpm');
    await p
      .getByRole('spinbutton', { name: /^(Lower threshold \(rpm\)|Нижний порог \(rpm\))$/ })
      .fill('2000');
    await p
      .getByRole('button', { name: /^(Apply sensor-driven mode|Применить режим по датчику)$/ })
      .click();
    const text = (en: boolean) =>
      en
        ? 'Thresholds must be integers from 0 to 3600, with the lower value no higher than the upper value'
        : 'Пороги должны быть целыми числами от 0 до 3600, нижний не выше верхнего';
    await expect(p.getByRole('alert')).toHaveText(text(initial === 'en-US'));
    await language(page, initial === 'en-US' ? 'ru-RU' : 'en-US');
    await expect(p.getByRole('alert')).toHaveText(text(initial !== 'en-US'));
    expect(await writes(page)).toEqual([]);
  });

for (const initial of ['en-US', 'ru-RU']) {
  test(`Fahrenheit integer-grid input keeps canonical RGB values and keyboard steps (${initial})`, async ({
    page,
  }) => {
    await setup(page, initial, 'fahrenheit');
    await openDynamic(page);
    const p = dynamic(page);
    const low = p.getByRole('spinbutton', {
        name: /^(Lower threshold \(°F\)|Нижний порог \(°F\))$/,
      }),
      high = p.getByRole('spinbutton', { name: /^(Upper threshold \(°F\)|Верхний порог \(°F\))$/ });
    await expect(low).toHaveAttribute('min', '32');
    await expect(low).toHaveAttribute('max', '212');
    await expect(low).toHaveAttribute('step', '1.8');
    await expect(low).toHaveAttribute('aria-describedby', 'dynamic-rgb-temperature-grid');
    await expect(high).toHaveAttribute('aria-describedby', 'dynamic-rgb-temperature-grid');
    await expect(p.locator('#dynamic-rgb-temperature-grid')).toBeVisible();
    await high.fill('212');
    for (const value of ['32', '33.8', '87.8', '210.2', '212']) {
      await low.fill(value);
      await expect(low).toHaveAttribute('aria-invalid', 'false');
      expect(await low.evaluate((input: HTMLInputElement) => input.validity.valid)).toBe(true);
    }
    await low.fill('87.8');
    await low.press('ArrowUp');
    await expect(low).toHaveValue('89.6');
    await high.fill('143.6');
    expect(await writes(page)).toEqual([]);
    await p
      .getByRole('button', { name: /^(Apply sensor-driven mode|Применить режим по датчику)$/ })
      .click();
    const requests = await page.evaluate(() =>
      (window as unknown as { rgbMessages: Fixture }).rgbMessages.requests.filter(
        (r) => r.method === 'set_dynamic_rgb',
      ),
    );
    expect(requests).toHaveLength(1);
    expect(requests[0].params.config).toMatchObject({
      unit: 'celsius',
      low_value: 32,
      high_value: 62,
    });
  });
  test(`Fahrenheit 100 input keeps accessible value consistent and rejects fractional Celsius without rounding (${initial})`, async ({
    page,
  }) => {
    await setup(page, initial, 'fahrenheit');
    await openDynamic(page);
    const p = dynamic(page),
      low = p.getByRole('spinbutton', { name: /^(Lower threshold \(°F\)|Нижний порог \(°F\))$/ });
    await low.fill('100');
    await expect(low).toHaveValue('100');
    await expect(low).toHaveAttribute('aria-valuenow', '100');
    await expect(low).toHaveAttribute('aria-invalid', 'true');
    expect(await low.evaluate((input: HTMLInputElement) => input.validity.stepMismatch)).toBe(true);
    await p
      .getByRole('button', { name: /^(Apply sensor-driven mode|Применить режим по датчику)$/ })
      .click();
    await expect(p.getByRole('alert')).toContainText(
      initial === 'en-US' ? 'whole degrees Celsius' : 'целым °C',
    );
    await expect(
      p.getByText(
        initial === 'en-US'
          ? 'This mode stores whole degrees Celsius. In Fahrenheit, use steps of 1.8° (for example, 86 → 87.8°F).'
          : 'Пороги этого режима хранятся целыми °C. В °F используйте шаг 1,8° (например, 86 → 87,8°F).',
        { exact: true },
      ),
    ).toBeVisible();
    await page.evaluate(() => {
      (window as unknown as { rgbMessages: Fixture }).rgbMessages.temperatureUnit = 'celsius';
    });
    const canonical = p.getByRole('spinbutton', {
      name: /^(Lower threshold \(°C\)|Нижний порог \(°C\))$/,
    });
    await expect(canonical).toHaveValue('37.77777778');
    await expect(canonical).toHaveAttribute('aria-invalid', 'true');
    await expect(canonical).not.toHaveAttribute('aria-describedby');
    await page.evaluate(() => {
      (window as unknown as { rgbMessages: Fixture }).rgbMessages.temperatureUnit = 'fahrenheit';
    });
    await expect(low).toHaveValue('100');
    expect(await writes(page)).toEqual([]);
  });
}
