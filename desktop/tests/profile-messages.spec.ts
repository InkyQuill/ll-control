import { test, expect } from '@playwright/test';

for (const initial of ['en-US', 'ru-RU'] as const) {
  for (const operation of ['list', 'rename'] as const) {
    test(`profile ${operation} preserves raw diagnostic and requires explicit readback (${initial})`, async ({ page }) => {
      await page.addInitScript(({ initial, operation }) => {
        localStorage.setItem('lconnect.interface-language', initial);
        const f = { language: initial as 'en-US' | 'ru-RU', settingsRevision: 0, failed: false, writes: [] as string[],
          library: { version: 1, revision: 0, profiles: [{ slot: 0, name: 'Настройки {1} <CPU>', content: {
            kind: 'screen', value: { version: 1, name: 'Original', background: [0, 0, 0], layers: [] },
          } }] } };
        Object.assign(window, { profileMessages: f, __TAURI_INTERNALS__: {
          invoke: async (_: string, req: { method: string; params: { edit: { action: string; name: string } } }) => {
        if (_ === 'desktop_set_language') return null;
            switch (req.method) {
              case 'get_display_settings': return { version: 1, revision: f.settingsRevision, temperature_unit: 'celsius', interface_language: f.language };
              case 'list_devices': return [{ id: 'lcd', display_name: 'My pump', family: 'galahad2-lcdv2' }];
              case 'list_sensors': return [];
              case 'get_lcd_state': return { source: 'Idle', requested: {}, last_error: null };
              case 'list_profiles':
                if (operation === 'list' && !f.failed) { f.failed = true; throw new Error('Настройки'); }
                return structuredClone(f.library);
              case 'edit_profile':
                f.writes.push(req.params.edit.action);
                f.library.profiles[0].name = req.params.edit.name; f.library.revision++;
                if (!f.failed) { f.failed = true; throw new Error('Настройки'); }
                return structuredClone(f.library);
              default: return null;
            }
          },
        } });
      }, { initial, operation });
      await page.goto('/?view=display');
      const english = initial === 'en-US';
      const title = page.getByLabel(english ? 'Name' : 'Название', { exact: true });
      await title.fill('Unsent LCD draft');
      await page.getByText(english ? 'Saved profiles' : 'Сохранённые профили', { exact: true }).click();
      const panel = page.locator('.profiles');
      await panel.getByRole('button', { name: english ? 'Refresh profile list' : 'Обновить список профилей', exact: true }).click();
      if (operation === 'rename') {
        await expect(panel.getByRole('combobox')).toContainText('Настройки {1} <CPU>');
        await expect(panel.getByRole('combobox')).toContainText(english ? 'Empty' : 'Пусто');
        await panel.getByLabel(english ? 'Profile name' : 'Имя профиля', { exact: true }).fill('Renamed {1} <GPU>');
        await panel.getByRole('button', { name: english ? 'Rename profile' : 'Переименовать профиль', exact: true }).click();
        await expect(panel.getByRole('button', { name: english ? 'Rename profile' : 'Переименовать профиль', exact: true })).toBeDisabled();
      }
      const context = (en: boolean) => operation === 'list'
        ? en ? 'Could not load the profile list:' : 'Не удалось загрузить список профилей:'
        : en ? 'Profile changes were not confirmed:' : 'Изменение профилей не подтверждено:';
      await expect(panel.getByRole('alert')).toContainText(context(english));
      await expect(panel.getByRole('alert')).toContainText('Настройки');
      await page.evaluate(() => {
        const f = (window as unknown as { profileMessages: { language: string; settingsRevision: number } }).profileMessages;
        f.language = f.language === 'en-US' ? 'ru-RU' : 'en-US'; f.settingsRevision++;
      });
      await expect(panel.getByRole('alert')).toContainText(context(!english));
      await expect(panel.getByRole('alert')).toContainText('Настройки');
      await expect(page.getByLabel(english ? 'Название' : 'Name', { exact: true })).toHaveValue('Unsent LCD draft');
      expect(await page.evaluate(() => (window as unknown as { profileMessages: { writes: string[] } }).profileMessages.writes)).toHaveLength(operation === 'rename' ? 1 : 0);
      await panel.getByRole('button', { name: english ? 'Обновить список профилей' : 'Refresh profile list', exact: true }).click();
      await expect(panel.getByRole('alert')).toHaveCount(0);
      await expect(panel.getByRole('combobox')).toContainText(operation === 'rename' ? 'Renamed {1} <GPU>' : 'Настройки {1} <CPU>');
      if (operation === 'rename') {
        await expect(panel.getByLabel(english ? 'Имя профиля' : 'Profile name', { exact: true })).toHaveValue('Renamed {1} <GPU>');
        await panel.getByRole('button', { name: english ? 'Переименовать профиль' : 'Rename profile', exact: true }).click();
      }
      expect(await page.evaluate(() => (window as unknown as { profileMessages: { writes: string[] } }).profileMessages.writes)).toHaveLength(operation === 'rename' ? 2 : 0);
    });
  }
}
