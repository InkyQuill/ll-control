import { test, expect, type Page } from '@playwright/test';
interface Fixture {
  language: string;
  diagnostic: string;
  failCommand: string;
  invalidResponse: string;
  keepIntegrationError: boolean;
  nativeCalls: string[];
  state: {
    revision: number;
    preferences: { closeToTray: boolean; startMinimized: boolean; autostart: boolean };
    registration: {
      present: boolean;
      enabled: boolean | null;
      managed: boolean;
      problem: string | null;
    };
    integrationError: string | null;
    trayReady: boolean;
    trayError: string | null;
    trayErrorCode?: string | null;
  };
  reads: number;
  saves: { expectedRevision: number; preferences: Fixture['state']['preferences'] }[];
  fail: boolean;
  exits: number;
}
async function setup(page: Page, language: 'en-US' | 'ru-RU' = 'ru-RU') {
  await page.addInitScript((language) => {
    localStorage.setItem('lconnect.interface-language', language);
    const state = {
      revision: 0,
      preferences: { closeToTray: true, startMinimized: false, autostart: false },
      registration: {
        present: false,
        enabled: false as boolean | null,
        managed: false,
        problem: null as string | null,
      },
      integrationError: null as string | null,
      trayReady: false,
      trayError: 'Tray host недоступен',
      trayErrorCode: null as string | null,
    };
    const f = {
      language,
      diagnostic: 'Настройки',
      failCommand: '',
      invalidResponse: '',
      keepIntegrationError: false,
      nativeCalls: [] as string[],
      state,
      reads: 0,
      saves: [] as { expectedRevision: number; preferences: typeof state.preferences }[],
      fail: false,
      exits: 0,
    };
    Object.assign(window, {
      desktopFixture: f,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          if (command === 'desktop_set_language') return null;
          if (['desktop_settings_get', 'desktop_settings_save', 'desktop_exit'].includes(command))
            f.nativeCalls.push(command);
          if (command === 'desktop_settings_get') f.reads++;
          if (command === f.failCommand) throw { message: f.diagnostic };
          if (command === f.invalidResponse) return null;
          if (command === 'desktop_settings_get')
            return {
              ...structuredClone(f.state),
              integrationError: f.keepIntegrationError ? f.state.integrationError : null,
            };
          if (command === 'desktop_settings_save') {
            const request = args as {
              expectedRevision: number;
              preferences: typeof state.preferences;
            };
            f.saves.push(structuredClone(request));
            if (request.expectedRevision !== state.revision)
              throw { message: 'stale desktop revision' };
            state.revision++;
            state.preferences = structuredClone(request.preferences);
            state.integrationError = f.fail ? 'Fixture registration failed' : null;
            state.registration = {
              present: state.preferences.autostart,
              enabled: f.fail ? null : state.preferences.autostart,
              managed: !f.fail,
              problem: f.fail ? 'Fixture existing entry preserved' : null,
            };
            return structuredClone(state);
          }
          if (command === 'desktop_exit') {
            f.exits++;
            return;
          }
          if (command === 'group_archive_status') return null;
          switch (args.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'get_display_settings':
              return {
                version: 1,
                revision: 0,
                temperature_unit: 'celsius',
                interface_language: f.language,
              };
            case 'list_devices':
              return [{ id: 'test', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
            case 'list_sensors':
            case 'list_prepared_clips':
            case 'list_theme_assets':
            case 'list_named_theme_assets':
              return [];
            case 'get_lcd_state':
              return {
                source: 'Idle',
                requested: { brightness_percent: 100, rotation_degrees: 0, mode: 'LocalUI' },
                last_error: null,
              };
            case 'list_profiles':
              return { version: 1, revision: 0, profiles: [] };
            default:
              return null;
          }
        },
      },
    });
  }, language);
  await page.goto('/?view=display');
  await expect(
    page.getByText(language === 'en-US' ? 'Daemon connected' : 'Daemon подключён', { exact: true }),
  ).toBeVisible({ timeout: 15000 });
  await page
    .getByRole('button', { name: language === 'en-US' ? 'Settings' : 'Настройки', exact: true })
    .click();
  await page
    .getByText(language === 'en-US' ? 'Application and autostart' : 'Приложение и автозапуск', {
      exact: true,
    })
    .click();
  await expect(
    page.getByRole('checkbox', {
      name:
        language === 'en-US'
          ? 'Minimize to tray when closing the window'
          : 'Сворачивать в tray при закрытии окна',
      exact: true,
    }),
  ).toBeVisible();
}
const checkbox = (page: Page) =>
  page.getByRole('checkbox', { name: 'Запускать приложение при входе в систему', exact: true });
test('desktop drafts survive polling and save camelCase native preferences independently of daemon', async ({
  page,
}) => {
  await setup(page);
  await checkbox(page).check();
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { desktopFixture: Fixture }).desktopFixture.reads),
    )
    .toBeGreaterThanOrEqual(2);
  await expect(checkbox(page)).toBeChecked();
  expect(
    await page.evaluate(
      () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.saves,
    ),
  ).toHaveLength(0);
  await page.getByRole('button', { name: 'Сохранить параметры приложения', exact: true }).click();
  await expect(page.getByText('Регистрация автозапуска: включена.', { exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.saves,
    ),
  ).toEqual([
    {
      expectedRevision: 0,
      preferences: { closeToTray: true, startMinimized: false, autostart: true },
    },
  ]);
});
test('durable intent with failed registration is not reported enabled and explicit retry uses readback revision', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate(() => {
    (window as unknown as { desktopFixture: Fixture }).desktopFixture.fail = true;
  });
  await checkbox(page).check();
  await page.getByRole('button', { name: 'Сохранить параметры приложения', exact: true }).click();
  await expect(
    page.getByText('Регистрация автозапуска: состояние не подтверждено.', { exact: true }),
  ).toBeVisible();
  await expect(checkbox(page)).toBeChecked();
  await expect(
    page.getByRole('alert').filter({ hasText: 'Fixture registration failed' }),
  ).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { desktopFixture: Fixture }).desktopFixture.reads),
    )
    .toBeGreaterThanOrEqual(2);
  await expect(
    page.getByRole('button', { name: 'Сохранить и повторить регистрацию', exact: true }),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as unknown as { desktopFixture: Fixture }).desktopFixture.fail = false;
  });
  await page
    .getByRole('button', { name: 'Сохранить и повторить регистрацию', exact: true })
    .click();
  await expect(page.getByText('Регистрация автозапуска: включена.', { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { desktopFixture: Fixture }).desktopFixture.saves.map(
        (s) => s.expectedRevision,
      ),
    ),
  ).toEqual([0, 1]);
});
test('external settings revision disables stale save and preserves draft until explicit reload', async ({
  page,
}) => {
  await setup(page);
  await checkbox(page).check();
  await page.evaluate(() => {
    const f = (window as unknown as { desktopFixture: Fixture }).desktopFixture;
    f.state.revision++;
    f.state.preferences.closeToTray = false;
  });
  await expect(
    page.getByRole('alert').filter({ hasText: 'Настройки изменились в другом окне' }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Сохранить параметры приложения', exact: true }),
  ).toBeDisabled();
  await expect(checkbox(page)).toBeChecked();
  await page.getByRole('button', { name: 'Обновить параметры приложения', exact: true }).click();
  await expect(checkbox(page)).not.toBeChecked();
  await expect(
    page.getByRole('button', { name: 'Сохранить параметры приложения', exact: true }),
  ).toBeEnabled();
});
test('full Exit is explicit and locks further desktop writes while native shutdown joins workers', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Выйти из приложения полностью', exact: true }).click();
  await expect(
    page.getByText('Завершаем активные операции перед выходом…', { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Сохранить параметры приложения', exact: true }),
  ).toBeDisabled();
  expect(
    await page.evaluate(
      () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.exits,
    ),
  ).toBe(1);
});

async function desktopLanguage(page: Page, language: string) {
  await page.evaluate((language) => {
    (window as unknown as { desktopFixture: Fixture }).desktopFixture.language = language;
  }, language);
  await expect(
    page.getByText(language === 'en-US' ? 'Application and autostart' : 'Приложение и автозапуск', {
      exact: true,
    }),
  ).toBeVisible();
}
for (const language of ['en-US', 'ru-RU'] as const) {
  const en = language === 'en-US';
  for (const context of ['read', 'save', 'exit'] as const)
    test(`desktop ${context} diagnostic stays raw and does not trigger writes (${language})`, async ({
      page,
    }) => {
      await setup(page, language);
      const draft = page.getByRole('checkbox', {
        name: en ? 'Start the application at login' : 'Запускать приложение при входе в систему',
        exact: true,
      });
      await draft.check();
      const command =
        context === 'read'
          ? 'desktop_settings_get'
          : context === 'save'
            ? 'desktop_settings_save'
            : 'desktop_exit';
      await page.evaluate((command) => {
        (window as unknown as { desktopFixture: Fixture }).desktopFixture.failCommand = command;
      }, command);
      await page
        .getByRole('button', {
          name:
            context === 'read'
              ? en
                ? 'Reload application settings'
                : 'Обновить параметры приложения'
              : context === 'save'
                ? en
                  ? 'Save application settings'
                  : 'Сохранить параметры приложения'
                : en
                  ? 'Exit application'
                  : 'Выйти из приложения полностью',
          exact: true,
        })
        .click();
      const alert = page.getByRole('alert').filter({ hasText: /^Настройки$/ });
      await expect(alert).toBeVisible();
      const reads = await page.evaluate(
        () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.reads,
      );
      await expect
        .poll(() =>
          page.evaluate(
            () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.reads,
          ),
        )
        .toBeGreaterThan(reads);
      await expect(alert).toBeVisible();
      await expect(draft).toBeChecked();
      await desktopLanguage(page, en ? 'ru-RU' : 'en-US');
      await expect(alert).toBeVisible();
      const calls = await page.evaluate(
        () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.nativeCalls,
      );
      expect(calls.filter((c) => c === 'desktop_settings_save')).toHaveLength(
        context === 'save' ? 1 : 0,
      );
      expect(calls.filter((c) => c === 'desktop_exit')).toHaveLength(context === 'exit' ? 1 : 0);
    });
  test(`desktop snapshot diagnostics remain data across language changes (${language})`, async ({
    page,
  }) => {
    await setup(page, language);
    await page.evaluate(() => {
      const f = (window as unknown as { desktopFixture: Fixture }).desktopFixture;
      f.keepIntegrationError = true;
      f.state.trayError = 'Настройки';
      f.state.registration.problem = 'Применить';
      f.state.integrationError = 'Библиотека';
    });
    await page
      .getByRole('button', {
        name: en ? 'Reload application settings' : 'Обновить параметры приложения',
        exact: true,
      })
      .click();
    for (const value of ['Настройки', 'Применить', 'Библиотека'])
      await expect(page.locator('details[open]').getByText(value, { exact: true })).toBeVisible();
    await desktopLanguage(page, en ? 'ru-RU' : 'en-US');
    for (const value of ['Настройки', 'Применить', 'Библиотека'])
      await expect(page.locator('details[open]').getByText(value, { exact: true })).toBeVisible();
    expect(
      await page.evaluate(
        () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.saves,
      ),
    ).toHaveLength(0);
  });
  for (const context of ['read', 'save'] as const)
    test(`desktop owned ${context} validation stays localized (${language})`, async ({ page }) => {
      await setup(page, language);
      await page.evaluate((context) => {
        (window as unknown as { desktopFixture: Fixture }).desktopFixture.invalidResponse =
          context === 'read' ? 'desktop_settings_get' : 'desktop_settings_save';
      }, context);
      await page
        .getByRole('button', {
          name:
            context === 'read'
              ? en
                ? 'Reload application settings'
                : 'Обновить параметры приложения'
              : en
                ? 'Save application settings'
                : 'Сохранить параметры приложения',
          exact: true,
        })
        .click();
      const expected = (english: boolean) =>
        context === 'read'
          ? english
            ? 'Application settings are unavailable.'
            : 'Настройки приложения недоступны.'
          : english
            ? 'The save response could not be confirmed. Reload the saved state.'
            : 'Ответ сохранения не подтверждён. Обновите состояние.';
      await expect(page.getByRole('alert').filter({ hasText: expected(en) })).toBeVisible();
      await desktopLanguage(page, en ? 'ru-RU' : 'en-US');
      await expect(page.getByRole('alert').filter({ hasText: expected(!en) })).toBeVisible();
    });
  test(`desktop saved notice changes language without saving again (${language})`, async ({
    page,
  }) => {
    await setup(page, language);
    await page
      .getByRole('button', {
        name: en ? 'Save application settings' : 'Сохранить параметры приложения',
        exact: true,
      })
      .click();
    await expect(
      page.getByText(en ? 'Application settings saved.' : 'Параметры приложения сохранены.', {
        exact: true,
      }),
    ).toBeVisible();
    await desktopLanguage(page, en ? 'ru-RU' : 'en-US');
    await expect(
      page.getByText(en ? 'Параметры приложения сохранены.' : 'Application settings saved.', {
        exact: true,
      }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.saves,
      ),
    ).toHaveLength(1);
  });
}

for (const language of ['en-US', 'ru-RU'] as const) {
  const en = language === 'en-US';
  for (const code of ['host_unconfirmed', 'unavailable'])
    test(`owned tray ${code} status follows language and keeps diagnostics raw (${language})`, async ({
      page,
    }) => {
      await setup(page, language);
      await page.evaluate((code) => {
        const f = (window as unknown as { desktopFixture: Fixture }).desktopFixture;
        f.state.trayErrorCode = code;
        f.state.trayError =
          code === 'host_unconfirmed'
            ? 'В сессии нет зарегистрированного tray host для приложения.'
            : 'Настройки';
      }, code);
      await page
        .getByRole('button', {
          name: en ? 'Reload application settings' : 'Обновить параметры приложения',
          exact: true,
        })
        .click();
      const own = (english: boolean) =>
        code === 'host_unconfirmed'
          ? english
            ? 'Tray host registration for this application is unconfirmed.'
            : 'Регистрация приложения в tray host не подтверждена.'
          : english
            ? 'Could not set up the tray icon.'
            : 'Не удалось подготовить значок tray.';
      const panel = page.locator('details[open]');
      await expect(panel.getByText(own(en), { exact: true })).toBeVisible();
      if (code === 'unavailable')
        await expect(panel.getByText('Настройки', { exact: true })).toBeVisible();
      await desktopLanguage(page, en ? 'ru-RU' : 'en-US');
      await expect(panel.getByText(own(!en), { exact: true })).toBeVisible();
      if (code === 'unavailable')
        await expect(panel.getByText('Настройки', { exact: true })).toBeVisible();
      expect(
        await page.evaluate(
          () => (window as unknown as { desktopFixture: Fixture }).desktopFixture.saves,
        ),
      ).toHaveLength(0);
    });
  for (const code of [null, 'future_code'])
    test(`legacy or unknown tray code stays raw (${language}, ${code})`, async ({ page }) => {
      await setup(page, language);
      const raw = 'Регистрация приложения в tray host не подтверждена.';
      await page.evaluate(
        ({ code, raw }) => {
          const f = (window as unknown as { desktopFixture: Fixture }).desktopFixture;
          if (code === null) delete f.state.trayErrorCode;
          else f.state.trayErrorCode = code;
          f.state.trayError = raw;
        },
        { code, raw },
      );
      await page
        .getByRole('button', {
          name: en ? 'Reload application settings' : 'Обновить параметры приложения',
          exact: true,
        })
        .click();
      await expect(page.locator('details[open]').getByText(raw, { exact: true })).toBeVisible();
      await desktopLanguage(page, en ? 'ru-RU' : 'en-US');
      await expect(page.locator('details[open]').getByText(raw, { exact: true })).toBeVisible();
    });
}
