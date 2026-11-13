import { test, expect, type Page } from '@playwright/test';
import type { GroupStatus } from '../src/lib/group-archive';
import { setup, panel, type Fixture } from './fixtures/group-archive';
async function calls(p: Page, action: string) {
  return p.evaluate(
    (a) =>
      (window as unknown as { groupFixture: Fixture }).groupFixture.calls.filter(
        (c) => c.args.action === a,
      ),
    action,
  );
}
const choose = (p: Page) =>
  panel(p).getByRole('button', { name: 'Выбрать архив группы…', exact: true });
test('inspection displays actual LCD values and zero PWM before explicit target-bound Apply; preserves editor', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Экран помпы', exact: true }).click();
  await page.getByLabel('Название', { exact: true }).fill('Мой черновик');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await choose(page).click();
  await expect(
    panel(page).getByText('LCD: 42% · 90° · Application', { exact: true }),
  ).toBeVisible();
  await expect(panel(page).getByText('Вентиляторы: 0%', { exact: true })).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(0);
  await panel(page)
    .getByRole('button', { name: 'Восстановить и применить группу', exact: true })
    .click();
  await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
  expect((await calls(page, 'finish'))[0].args.request).toMatchObject({
    dev: 'test',
    group: 'screen',
    mode: 'import',
  });
  await page.getByRole('button', { name: 'Экран помпы', exact: true }).click();
  await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Мой черновик');
});
test('export stays in Rust, has explicit fan+lighting target and reports completion', async ({
  page,
}) => {
  await setup(page);
  await panel(page).getByLabel('Группа настроек', { exact: true }).selectOption('fan_and_lighting');
  await panel(page).getByRole('button', { name: 'Экспортировать группу…', exact: true }).click();
  await expect(panel(page).getByRole('progressbar')).toBeVisible();
  const start = await page.evaluate(
    () =>
      (window as unknown as { groupFixture: Fixture }).groupFixture.calls.find(
        (c) => c.command === 'group_archive_start',
      )!.args,
  );
  expect(Object.keys(start).sort()).toEqual(['language', 'request']);
  expect(start.language).toBe('ru-RU');
  expect(start.request).toMatchObject({ dev: 'test', group: 'fan_and_lighting', mode: 'export' });
  expect(JSON.stringify(start)).not.toContain('bytes');
  await page.evaluate(
    () => ((window as unknown as { groupFixture: Fixture }).groupFixture.job!.phase = 'complete'),
  );
  await expect(
    panel(page).getByText('Архив группы сохранён в файл.', { exact: true }),
  ).toBeVisible();
});
test('prepared import cannot be dismissed, cancelled worker remains busy until acknowledgement', async ({
  page,
}) => {
  await setup(page);
  await choose(page).click();
  await expect(
    panel(page).getByRole('button', { name: 'Закрыть сведения о группе', exact: true }),
  ).toBeDisabled();
  await panel(page).getByRole('button', { name: 'Отменить передачу группы', exact: true }).click();
  await expect(panel(page).getByRole('progressbar')).toBeVisible();
  await expect(choose(page)).toBeDisabled();
  await page.evaluate(
    () => ((window as unknown as { groupFixture: Fixture }).groupFixture.job!.phase = 'cancelled'),
  );
  await expect(
    panel(page).getByRole('button', { name: 'Закрыть сведения о группе', exact: true }),
  ).toBeEnabled();
  await panel(page).getByRole('button', { name: 'Закрыть сведения о группе', exact: true }).click();
  await expect(choose(page)).toBeEnabled();
});
test('lost Start response recovers preview by polling without automatic Finish or duplicate upload', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate(
    () => ((window as unknown as { groupFixture: Fixture }).groupFixture.lost = true),
  );
  await choose(page).click();
  await expect(
    panel(page).getByRole('button', { name: 'Восстановить и применить группу', exact: true }),
  ).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(0);
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { groupFixture: Fixture }).groupFixture.calls.filter(
          (c) => c.command === 'group_archive_start',
        ).length,
    ),
  ).toBe(1);
});
test('stored ID/device/group survives reload and readback never repeats Finish', async ({
  page,
}) => {
  await setup(page);
  await choose(page).click();
  const pending = await page.evaluate(() =>
    JSON.parse(localStorage.getItem('llcontrol.group.pending')!),
  );
  await page.reload();
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByText('Резервная копия группы настроек', { exact: true }).click();
  await expect(panel(page).getByLabel('ID операции группы', { exact: true })).toHaveValue(
    pending.id,
  );
  await panel(page)
    .getByRole('button', { name: 'Прочитать результат группы', exact: true })
    .click();
  await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(0);
  expect((await calls(page, 'recover'))[0].args.request).toEqual(pending);
});
test('reapply echoes opaque revision and a later content change disables another Apply', async ({
  page,
}) => {
  await setup(page);
  await choose(page).click();
  await panel(page)
    .getByRole('button', { name: 'Восстановить и применить группу', exact: true })
    .click();
  await panel(page)
    .getByRole('button', { name: 'Повторить применение сохранённой группы', exact: true })
    .click();
  expect((await calls(page, 'reapply'))[0].args.expected).toBe('opaque runtime token');
  await page.evaluate(() => {
    const j = (window as unknown as { groupFixture: Fixture }).groupFixture.job!;
    j.readback!.selected_content_current = false;
  });
  await expect(
    panel(page).getByRole('button', {
      name: 'Повторить применение сохранённой группы',
      exact: true,
    }),
  ).toBeDisabled();
});
test('receipt with Apply error distinguishes committed settings from runtime recovery', async ({
  page,
}) => {
  await setup(page);
  await choose(page).click();
  await panel(page)
    .getByRole('button', { name: 'Восстановить и применить группу', exact: true })
    .click();
  await page.evaluate(() => {
    const j = (window as unknown as { groupFixture: Fixture }).groupFixture.job!;
    j.readback!.runtime.last_error = 'Mock LCD frame failure';
    j.readback!.runtime.recovery_required = true;
  });
  await expect(
    panel(page).getByRole('alert').filter({ hasText: 'Mock LCD frame failure' }),
  ).toBeVisible();
  await expect(
    panel(page).getByRole('button', {
      name: 'Повторить применение сохранённой группы',
      exact: true,
    }),
  ).toBeDisabled();
});
test('another persisted operation cannot be replaced by a new Start', async ({ page }) => {
  await setup(page);
  await page.evaluate(() =>
    localStorage.setItem(
      'llcontrol.group.pending',
      JSON.stringify({ id: 'b'.repeat(32), dev: 'other-device', group: 'screen', mode: 'import' }),
    ),
  );
  await choose(page).click();
  await expect(panel(page).getByRole('alert')).toContainText('Сначала прочитайте');
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { groupFixture: Fixture }).groupFixture.calls.filter(
          (c) => c.command === 'group_archive_start',
        ).length,
    ),
  ).toBe(0);
});

test('changing selected device after Inspect cannot redirect Apply to another device', async ({
  page,
}) => {
  await setup(page);
  await choose(page).click();
  await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('other');
  await expect(
    panel(page).getByRole('button', { name: 'Восстановить и применить группу', exact: true }),
  ).toBeDisabled();
  expect(await calls(page, 'finish')).toHaveLength(0);
  await expect(panel(page).getByText('test', { exact: true })).toBeVisible();
});

test('full reset requires temperature selection and separate confirmation; preview cancel never applies', async ({
  page,
}) => {
  await setup(page);
  await panel(page)
    .getByRole('combobox', { name: 'Группа настроек', exact: true })
    .selectOption('fan_and_lighting');
  const start = panel(page).getByRole('button', {
    name: 'Подготовить полный сброс охлаждения и подсветки…',
    exact: true,
  });
  await expect(start).toBeDisabled();
  await panel(page)
    .getByRole('combobox', { name: 'Температурный датчик CPU для сброса', exact: true })
    .selectOption('cpu:test');
  await start.click();
  await expect(
    panel(page).getByText('Сброс подготовлен, ожидает подтверждения', { exact: true }),
  ).toBeVisible();
  const admitted = await page.evaluate(() =>
    (window as unknown as { groupFixture: Fixture }).groupFixture.calls.filter(
      (c) => c.command === 'group_archive_start',
    ),
  );
  expect(admitted).toHaveLength(1);
  expect(admitted[0].args.request).toMatchObject({
    mode: 'reset',
    group: 'fan_and_lighting',
    sensor_id: 'cpu:test',
  });
  expect(await calls(page, 'finish')).toHaveLength(0);
  await panel(page)
    .getByRole('button', { name: 'Отменить подготовку сброса', exact: true })
    .click();
  expect(await calls(page, 'finish')).toHaveLength(0);
});

test('full reset readback survives reload without repeating reset; newer content disables reapply', async ({
  page,
}) => {
  await setup(page);
  await panel(page)
    .getByRole('combobox', { name: 'Группа настроек', exact: true })
    .selectOption('fan_and_lighting');
  await panel(page)
    .getByRole('combobox', { name: 'Температурный датчик CPU для сброса', exact: true })
    .selectOption('cpu:test');
  await panel(page)
    .getByRole('button', { name: 'Подготовить полный сброс охлаждения и подсветки…', exact: true })
    .click();
  await panel(page)
    .getByRole('button', { name: 'Подтвердить полный сброс и применить', exact: true })
    .click();
  await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
  const pending = await page.evaluate(() =>
    JSON.parse(localStorage.getItem('llcontrol.group.pending')!),
  );
  expect(pending.mode).toBe('reset');
  await page.reload();
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByText('Резервная копия группы настроек', { exact: true }).click();
  await panel(page)
    .getByRole('button', { name: 'Прочитать результат группы', exact: true })
    .click();
  await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(0);
  await page.evaluate(() => {
    (
      window as unknown as { groupFixture: Fixture }
    ).groupFixture.job!.readback!.selected_content_current = false;
  });
  await expect(
    panel(page).getByRole('button', {
      name: 'Повторить применение сохранённой группы',
      exact: true,
    }),
  ).toBeDisabled();
});

test('lost reset reply offers only explicit retry with the original operation identity', async ({
  page,
}) => {
  await setup(page);
  await panel(page)
    .getByRole('combobox', { name: 'Группа настроек', exact: true })
    .selectOption('fan_and_lighting');
  await panel(page)
    .getByRole('combobox', { name: 'Температурный датчик CPU для сброса', exact: true })
    .selectOption('cpu:test');
  await panel(page)
    .getByRole('button', { name: 'Подготовить полный сброс охлаждения и подсветки…', exact: true })
    .click();
  await page.evaluate(() => {
    (window as unknown as { groupFixture: Fixture }).groupFixture.lostFinish = true;
  });
  await panel(page)
    .getByRole('button', { name: 'Подтвердить полный сброс и применить', exact: true })
    .click();
  const retry = panel(page).getByRole('button', {
    name: 'Повторить запрос сброса с тем же ID',
    exact: true,
  });
  await expect(retry).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(1);
  await expect(
    panel(page).getByRole('button', { name: 'Отменить подготовку сброса', exact: true }),
  ).toHaveCount(0);
  await retry.click();
  await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
  const sent = await calls(page, 'finish');
  expect(sent).toHaveLength(2);
  expect(sent[0].args.request).toEqual(sent[1].args.request);
});

test('English group phases and parameterized curves switch to Russian without translating stored names or applying', async ({
  page,
}) => {
  await setup(page, 'en-US');
  const english = page.getByText('Settings group backup', { exact: true }).locator('..');
  await english.getByRole('button', { name: 'Choose group archive…', exact: true }).click();
  await expect(
    english.getByRole('status').filter({ hasText: 'Archive checked; waiting for Apply' }),
  ).toBeVisible();
  await page.evaluate(() => {
    const f = (window as unknown as { groupFixture: Fixture }).groupFixture;
    const summary = f.job!.summary!;
    summary.slots[0].name = 'Настройки';
    summary.theme_name = 'Экран';
    summary.cooling = {
      fan: {
        mode: 'curve',
        sensor_id: 'Настройки {1} <CPU>',
        points: [
          { celsius: 30, percent: 40 },
          { celsius: 70, percent: 100 },
        ],
        hysteresis_c: 2,
        fallback_percent: 100,
      },
      pump: null,
    };
  });
  await expect(
    english.getByText('Fan policy: PWM curve · Настройки {1} <CPU> · points: 2', { exact: true }),
  ).toBeVisible();
  await expect(english.getByText('Pump policy: not configured', { exact: true })).toBeVisible();
  await expect(english.getByText('screen · 8: Настройки', { exact: true })).toBeVisible();
  await expect(english.getByText('Theme: Экран', { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const f = (window as unknown as { groupFixture: Fixture & { language: string } }).groupFixture;
    f.job!.phase = 'downloading';
    f.language = 'ru-RU';
  });
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru');
  await expect(
    panel(page).getByRole('status').filter({ hasText: 'Сохранение файла' }),
  ).toBeVisible();
  await expect(
    panel(page).getByRole('progressbar', { name: 'Передача группы', exact: true }),
  ).toBeVisible();
  await expect(
    panel(page).getByText('Политика вентиляторов: Кривая PWM · Настройки {1} <CPU> · точек: 2', {
      exact: true,
    }),
  ).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(0);
  expect(await calls(page, 'reapply')).toHaveLength(0);
  const startArgs = await page.evaluate(
    () =>
      (window as unknown as { groupFixture: Fixture }).groupFixture.calls.find(
        (c) => c.command === 'group_archive_start',
      )!.args,
  );
  expect(startArgs).toMatchObject({ language: 'en-US' });
});

test('typed group action rejection preserves raw diagnostic and never infers notice codes from text', async ({
  page,
}) => {
  await setup(page, 'en-US');
  const english = page.getByText('Settings group backup', { exact: true }).locator('..');
  await english.getByRole('button', { name: 'Choose group archive…', exact: true }).click();
  await page.evaluate(() => {
    const w = window as unknown as {
      __TAURI_INTERNALS__: {
        invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
      };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'group_archive_action')
        throw { message: 'Настройки {1} <CPU>', message_code: 'group_export_unconfirmed' };
      return original(command, args);
    };
  });
  await english.getByRole('button', { name: 'Restore and apply group', exact: true }).click();
  await expect(
    english
      .getByRole('alert')
      .filter({ hasText: 'The local file is unconfirmed. Cancel the transfer and export again.' }),
  ).toBeVisible();
  await english.getByText('Technical diagnostics', { exact: true }).click();
  await expect(english.locator('pre')).toHaveText('Настройки {1} <CPU>');
  await page.evaluate(() => {
    const j = (window as unknown as { groupFixture: Fixture }).groupFixture.job!;
    j.phase = 'failed';
    j.message = 'Настройки';
    j.message_code = 'future_code';
  });
  await expect(english.getByRole('alert').filter({ hasText: 'Настройки' })).toBeVisible();
  expect(await calls(page, 'finish')).toHaveLength(0);
});

for (const language of ['en-US', 'ru-RU']) {
  test(`${language}: full reset preview exposes all factory modes and selected IDs before confirmation`, async ({
    page,
  }) => {
    await setup(page, language);
    const en = language === 'en-US',
      area = page
        .locator('details')
        .filter({
          has: page
            .locator('summary')
            .filter({ hasText: en ? 'Settings group backup' : 'Резервная копия группы настроек' }),
        })
        .first();
    await area
      .getByRole('combobox', { name: en ? 'Settings group' : 'Группа настроек', exact: true })
      .selectOption('fan_and_lighting');
    await area
      .getByRole('combobox', {
        name: en ? 'CPU temperature sensor for reset' : 'Температурный датчик CPU для сброса',
        exact: true,
      })
      .selectOption('cpu:test');
    await area
      .getByRole('button', {
        name: en
          ? 'Prepare full cooling and lighting reset…'
          : 'Подготовить полный сброс охлаждения и подсветки…',
        exact: true,
      })
      .click();
    await page.evaluate(() => {
      const f = (window as unknown as { groupFixture: Fixture }).groupFixture;
      const summary = f.job!.summary!;
      summary.slots = Array.from({ length: 8 }, (_, i) => ({
        group: i < 4 ? 'fan' : 'pump',
        slot: i % 4,
        name: `Slot ${(i % 4) + 1}`,
      }));
      summary.lcd = null;
      summary.theme_name = null;
      summary.fan_manual_percent = null;
      summary.pump_manual = null;
      summary.cooling = {
        fan_mode: 'StandardSpeed',
        pump_mode: 'PWMQuiet',
        fan: {
          mode: 'fan_curve',
          sensor_id: 'cpu:test',
          points: [
            { celsius: 0, percent: 10 },
            { celsius: 100, percent: 100 },
          ],
          hysteresis_c: 0,
          fallback_percent: 100,
        },
        pump: {
          mode: 'curve_rpm',
          sensor_id: 'cpu:test',
          points: [
            { celsius: 30, rpm: 2000 },
            { celsius: 100, rpm: 3600 },
          ],
          hysteresis_c: 0,
          fallback_percent: 100,
        },
      };
      summary.cooling_modes = {
        fan: [
          'Quiet',
          'StandardSpeed',
          'HighSpeed',
          'FullSpeed',
          'UserDefine1',
          'UserDefine2',
          'UserDefine3',
          'UserDefine4',
        ].map((id) => ({ id, name: id, policy: { mode: 'fixed', percent: 50 } })),
        pump: ['FixRPM', 'PWMPerformance', 'PWMQuiet'].map((id) => ({
          id,
          name: id,
          policy: { mode: 'fixed_rpm', rpm: 2200 },
        })),
      };
    });
    const bank = area.locator('details.bank-summary');
    await expect(bank).toBeVisible();
    await bank.locator('summary').click();
    await expect(bank.locator('p')).toHaveCount(11);
    await expect(area.getByText(/StandardSpeed,.*PWMQuiet/)).toBeVisible();
    expect(await calls(page, 'finish')).toHaveLength(0);
    for (const [width, height] of [
      [1280, 1000],
      [1024, 900],
    ]) {
      await page.setViewportSize({ width, height });
      await bank.scrollIntoViewIfNeeded();
      expect(await area.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
      await page.screenshot({
        path: `../.impeccable/review/control-reset-bank-${language}-${width}.png`,
      });
    }
  });
}

test('Windows import experimental disclosure explains unverified behavior in EN/RU', async ({
  page,
}) => {
  await setup(page);
  const disclosure = page
    .locator('details')
    .filter({ hasText: 'Windows ZIP' })
    .locator('[data-feature="windows-import"]');
  await expect(disclosure).toContainText('Экспериментально');
  await disclosure
    .getByRole('button', { name: 'Ограничения экспериментальной функции', exact: true })
    .click();
  await expect(
    page.getByRole('dialog', { name: 'Ограничения экспериментальной функции', exact: true }),
  ).toContainText('не подтверждён для всех экспортов L-Connect 2.1.28');
  await page.keyboard.press('Escape');
  await page.evaluate(() => {
    (window as unknown as { groupFixture: Fixture }).groupFixture.language = 'en-US';
  });
  await expect(disclosure).toContainText('Experimental');
  await disclosure
    .getByRole('button', { name: 'Experimental feature limitations', exact: true })
    .click();
  await expect(
    page.getByRole('dialog', { name: 'Experimental feature limitations', exact: true }),
  ).toContainText('not verified for every L-Connect 2.1.28 export');
});
