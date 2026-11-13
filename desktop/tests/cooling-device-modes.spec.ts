import { test, expect, type Page } from '@playwright/test';

type Fixture = {
  calls: { method: string; params: Record<string, unknown> }[];
  payload: Record<string, unknown>;
  hold: boolean;
  releases: (() => void)[];
  error: string;
  saveFault?: 'lost' | 'pending' | 'conflict';
  holdSave?: boolean;
  holdDefault?: boolean;
  defaultFault?: 'invalid' | 'error';
  online?: boolean;
  settings: {
    version: number;
    revision: number;
    temperature_unit: 'celsius';
    interface_language: 'en-US' | 'ru-RU';
  };
};
const copy = {
  'en-US': {
    cooling: 'Cooling',
    summary: 'Saved device modes',
    read: 'Read device modes',
    mode: 'Saved mode',
    load: 'Load mode into draft',
    apply: 'Apply cooling',
    target: 'Channel',
    invalid: 'The daemon returned invalid saved cooling modes',
    empty: 'No saved modes for this device yet',
  },
  'ru-RU': {
    cooling: 'Охлаждение',
    summary: 'Сохранённые режимы устройства',
    read: 'Прочитать режимы устройства',
    mode: 'Сохранённый режим',
    load: 'Загрузить режим в черновик',
    apply: 'Применить охлаждение',
    target: 'Канал',
    invalid: 'Daemon вернул некорректные сохранённые режимы охлаждения',
    empty: 'Для этого устройства пока нет сохранённых режимов',
  },
};

async function setup(page: Page, language: 'en-US' | 'ru-RU' = 'ru-RU') {
  await page.addInitScript((language) => {
    localStorage.setItem('llcontrol.interface-language', language);
    const bank = {
      version: 1,
      revision: '1'.repeat(64),
      persistence: { pending: false, saved: false, last_error: null as string | null },
      fan: Array.from({ length: 8 }, (_, i) => ({
        id: `F${i}`,
        name: `Сохранённый ${i}`,
        policy: {
          mode: 'fan_curve',
          sensor_id: i === 3 ? 'cpu:missing' : 'cpu:test',
          points: [
            { celsius: 0, percent: 10 },
            { celsius: 40, percent: 50 + i },
            { celsius: 100, percent: 100 },
          ],
          hysteresis_c: 0,
          fallback_percent: 97,
        },
      })),
      pump: [
        { id: 'P0', name: 'Настройки', policy: { mode: 'fixed_rpm', rpm: 2400 } },
        {
          id: 'P1',
          name: 'Моя кривая',
          policy: {
            mode: 'curve_rpm',
            sensor_id: 'cpu:test',
            points: [
              { celsius: 0, rpm: 2000 },
              { celsius: 100, rpm: 3000 },
            ],
            hysteresis_c: 1,
            fallback_percent: 96,
          },
        },
        { id: 'P2', name: 'Плата', policy: { mode: 'motherboard', percent: 80 } },
      ],
    };
    const other = structuredClone(bank);
    other.pump.forEach((mode) => (mode.name = `B ${mode.name}`));
    const f: Fixture = {
      calls: [],
      payload: { A: bank, B: other },
      hold: false,
      releases: [],
      error: '',
      settings: {
        version: 1,
        revision: 0,
        temperature_unit: 'celsius',
        interface_language: language,
      },
    };
    const makeDefaults = (sensor: string) => {
      const ids = [
        'Quiet',
        'StandardSpeed',
        'HighSpeed',
        'FullSpeed',
        'UserDefine1',
        'UserDefine2',
        'UserDefine3',
        'UserDefine4',
      ];
      const names = [
        'Quiet',
        'Standard',
        'High speed',
        'Full speed',
        'Custom 1',
        'Custom 2',
        'Custom 3',
        'Custom 4',
      ];
      return {
        ...structuredClone(bank),
        revision: '2'.repeat(64),
        persistence: { pending: f.saveFault === 'pending', saved: true, last_error: null },
        fan: bank.fan.map((mode, i) => ({
          ...structuredClone(mode),
          id: ids[i],
          name: names[i],
          use_default_name: true,
          policy: { ...structuredClone(mode.policy), sensor_id: sensor },
        })),
        pump: [
          {
            id: 'FixRPM',
            name: 'Fixed RPM',
            use_default_name: true,
            policy: { mode: 'fixed_rpm', rpm: 2200 },
          },
          ...[
            ['PWMPerformance', 'Performance'],
            ['PWMQuiet', 'Quiet'],
          ].map(([id, name]) => ({
            id,
            name,
            use_default_name: true,
            policy: { ...structuredClone(bank.pump[1].policy), sensor_id: sensor },
          })),
        ],
      };
    };
    const channel = {
      mode_id: undefined as string | undefined,
      requested: null as unknown,
      applied_percent: 80,
      motherboard: false,
      fallback_active: false,
      last_error: null,
    };
    const state = { fan: structuredClone(channel), pump: structuredClone(channel) };
    Object.assign(window, {
      coolingModesFixture: f,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          if (command === 'cooling_mode_default') {
            f.calls.push({ method: 'cooling_mode_default', params: structuredClone(args) });
            const bank = makeDefaults(String(args.sensorId ?? '')),
              target = args.target as 'fan' | 'pump';
            const profile = bank[target].find((mode) => mode.id === args.id);
            if (!profile) throw new Error('unknown factory mode');
            if (f.holdDefault) await new Promise<void>((resolve) => f.releases.push(resolve));
            if (f.defaultFault === 'error') throw new Error('factory transport detail');
            return f.defaultFault === 'invalid'
              ? { ...profile, name: 'Wrong factory label' }
              : structuredClone(profile);
          }
          if (command !== 'daemon_call') return null;
          const method = args.method as string,
            params = (args.params ?? {}) as Record<string, unknown>;
          f.calls.push({ method, params: structuredClone(params) });
          switch (method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'get_display_settings':
              return structuredClone(f.settings);
            case 'list_devices':
              return f.online === false
                ? []
                : [
                    { id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' },
                    { id: 'B', display_name: 'LCD B', family: 'galahad2-lcdv2' },
                  ];
            case 'list_sensors':
              return [
                {
                  id: 'cpu:test',
                  label: 'CPU package',
                  unit: 'celsius',
                  value: 42,
                  status: 'valid',
                  age_ms: 1,
                },
              ];
            case 'get_lcd_state':
              return {
                source: 'Idle',
                requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 },
                last_error: null,
              };
            case 'list_named_theme_assets':
            case 'list_theme_assets':
            case 'list_prepared_clips':
            case 'list_theme_fonts':
              return [];
            case 'list_profiles':
              return { version: 1, revision: 0, profiles: [] };
            case 'get_cooling_state':
              return structuredClone(state);
            case 'get_cooling_modes': {
              if (f.error) throw { message: f.error };
              const result = structuredClone(f.payload[params.dev as string]);
              return f.hold
                ? await new Promise((resolve) => f.releases.push(() => resolve(result)))
                : result;
            }
            case 'initialize_cooling_modes': {
              if (f.payload[params.dev as string] !== null) throw new Error('already exists');
              const result = makeDefaults(String(params.sensor));
              f.payload[params.dev as string] = result;
              if (f.holdSave) await new Promise<void>((resolve) => f.releases.push(resolve));
              if (f.saveFault === 'lost') throw new Error('lost creation reply');
              return structuredClone(result);
            }
            case 'save_cooling_mode': {
              type Mode = { id: string; name: string; policy: unknown };
              const bank = f.payload[params.dev as string] as {
                revision: string;
                fan: Mode[];
                pump: Mode[];
                persistence: { pending: boolean; saved: boolean; last_error: string | null };
              };
              if (f.saveFault === 'conflict') {
                bank.revision = 'a'.repeat(64);
                bank.pump[0].name = 'New remote name';
                throw new Error('conflict');
              }
              if (bank.revision !== params.expected_revision) throw new Error('conflict');
              const modes = bank[params.target as 'fan' | 'pump'],
                profile = params.profile as Mode;
              modes[modes.findIndex((mode) => mode.id === profile.id)] = structuredClone(profile);
              bank.revision = (BigInt('0x' + bank.revision) + 1n).toString(16).padStart(64, '0');
              bank.persistence = {
                pending: f.saveFault === 'pending',
                saved: true,
                last_error: null,
              };
              if (f.holdSave) await new Promise<void>((resolve) => f.releases.push(resolve));
              if (f.saveFault === 'lost') throw new Error('lost acknowledgement');
              return structuredClone(bank);
            }
            case 'apply_cooling_mode': {
              const bank = f.payload[params.dev as string] as {
                revision: string;
                fan: { id: string; policy: unknown }[];
                pump: { id: string; policy: unknown }[];
              };
              if (bank.revision !== params.expected_revision)
                throw new Error('cooling modes changed');
              const target = params.target as 'fan' | 'pump',
                mode = bank[target].find((mode) => mode.id === params.id);
              if (!mode) throw new Error('missing mode');
              state[target] = {
                ...channel,
                requested: structuredClone(mode.policy),
                applied_percent: 57,
                mode_id: mode.id,
              };
              return structuredClone(state);
            }
            case 'set_cooling_policy': {
              const target = params.target;
              if (target !== 'fan' && target !== 'pump') throw new Error('bad target');
              state[target] = { ...channel, requested: params.policy, applied_percent: 57 };
              return structuredClone(state);
            }
            default:
              return null;
          }
        },
      },
    });
  }, language);
  await page.goto('/?view=display');
  await page.getByRole('button', { name: copy[language].cooling, exact: true }).click();
  await expect(
    page.getByText(language === 'en-US' ? 'Applied PWM: 80%' : 'Применённый PWM: 80%'),
  ).toBeVisible();
}

for (const kind of ['local', 'remote'] as const) {
  test(`${kind} mode-list diagnostic follows language changes without translating remote data`, async ({
    page,
  }) => {
    await setup(page, 'ru-RU');
    await page.getByRole('spinbutton', { name: 'PWM, %', exact: true }).fill('77');
    await page.evaluate((kind) => {
      const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
      if (kind === 'remote') f.error = 'Сохранённые режимы устройства';
      else f.payload.A = { version: 2, fan: [], pump: [] };
    }, kind);
    const panel = await open(page, 'ru-RU');
    await expect(panel.getByRole('alert')).toBeVisible();
    await page.evaluate(async () => {
      const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
      f.settings.interface_language = 'en-US';
      f.settings.revision++;
      const modulePath = '/src/lib/presentation.svelte.ts';
      const { publishDisplaySettings } = await import(/* @vite-ignore */ modulePath);
      publishDisplaySettings(f.settings);
    });
    await expect(
      panel.getByRole('button', { name: 'Read device modes', exact: true }),
    ).toBeVisible();
    await expect(panel.getByRole('alert')).toContainText('Could not read cooling modes:');
    await expect(panel.getByRole('alert')).toContainText(
      kind === 'local' ? copy['en-US'].invalid : 'Сохранённые режимы устройства',
    );
    await expect(page.getByRole('spinbutton', { name: 'PWM, %', exact: true })).toHaveValue('77');
    expect(await writes(page)).toHaveLength(0);
  });
}
const writes = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
      (c) => c.method === 'set_cooling_policy' || c.method === 'apply_cooling_mode',
    ),
  );
async function open(page: Page, language: 'en-US' | 'ru-RU') {
  const panel = page.locator('details.device-modes');
  await panel.locator('summary').focus();
  await panel.locator('summary').press('Enter');
  await expect(panel).toHaveAttribute('open', '');
  await panel.getByRole('button', { name: copy[language].read, exact: true }).click();
  return panel;
}

for (const language of ['en-US', 'ru-RU'] as const) {
  test(`${language}: all modes load as independent drafts and only Apply writes the chosen channel`, async ({
    page,
  }) => {
    await setup(page, language);
    const c = copy[language],
      panel = await open(page, language);
    const select = panel.getByRole('combobox', { name: c.mode, exact: true });
    await expect(select.locator('option')).toHaveCount(4);
    await expect(panel.getByRole('button', { name: c.read, exact: true })).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(select).toBeFocused();
    await select.selectOption('P1');
    await panel.getByRole('button', { name: c.load, exact: true }).press('Enter');
    await expect(
      page.getByRole('combobox', { name: language === 'en-US' ? 'Mode' : 'Режим', exact: true }),
    ).toHaveValue('curve_rpm');
    const rpm = page.getByRole('spinbutton', { name: 'RPM', exact: true });
    await expect(rpm.first()).toHaveValue('2000');
    await rpm.first().fill('2300');
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    await expect(rpm.first()).toHaveValue('2000');
    expect(await writes(page)).toHaveLength(0);
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    let calls = await writes(page);
    expect(calls).toHaveLength(1);
    expect(calls[0]).toMatchObject({
      method: 'apply_cooling_mode',
      params: { dev: 'A', target: 'pump', id: 'P1', expected_revision: '1'.repeat(64) },
    });
    await page.getByRole('combobox', { name: c.target, exact: true }).selectOption('fan');
    await expect(select).toHaveCount(0);
    await panel.getByRole('button', { name: c.read, exact: true }).click();
    await expect(select.locator('option')).toHaveCount(9);
    await select.selectOption('F3');
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    const sensor = page.getByRole('combobox', {
      name: language === 'en-US' ? 'Temperature source' : 'Источник температуры',
      exact: true,
    });
    await expect(sensor).toHaveValue('cpu:missing');
    expect(await writes(page)).toHaveLength(1);
    await sensor.selectOption('cpu:test');
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    calls = await writes(page);
    expect(calls).toHaveLength(2);
    expect(calls[1].params).toMatchObject({
      dev: 'A',
      target: 'fan',
      policy: {
        mode: 'fan_curve',
        sensor_id: 'cpu:test',
        fallback_percent: 97,
        points: [
          { celsius: 0, percent: 10 },
          { celsius: 40, percent: 53 },
          { celsius: 100, percent: 100 },
        ],
      },
    });
  });

  test(`${language}: late mode list cannot overwrite another device and refresh does not alter a draft`, async ({
    page,
  }) => {
    await setup(page, language);
    const c = copy[language];
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.hold = true),
    );
    const panel = await open(page, language);
    await expect(panel.getByRole('button')).toHaveAttribute('aria-busy', 'true');
    await page
      .getByRole('combobox', { name: language === 'en-US' ? 'Device' : 'Устройство', exact: true })
      .selectOption('B');
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.hold = false),
    );
    await panel.getByRole('button', { name: c.read, exact: true }).click();
    await expect(
      panel.getByRole('combobox', { name: c.mode, exact: true }).locator('option'),
    ).toHaveCount(4);
    await page.evaluate(() =>
      (
        window as unknown as { coolingModesFixture: Fixture }
      ).coolingModesFixture.releases.shift()?.(),
    );
    await expect(panel.getByRole('option', { name: 'B Настройки (P0)', exact: true })).toHaveCount(
      1,
    );
    await expect(panel.getByRole('option', { name: 'Настройки (P0)', exact: true })).toHaveCount(0);
    expect(await writes(page)).toHaveLength(0);
  });

  test(`${language}: a confirmed empty bank is distinct from read errors`, async ({ page }) => {
    await setup(page, language);
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.payload.A =
          null),
    );
    const panel = await open(page, language);
    await expect(panel.getByRole('status')).toContainText(copy[language].empty);
    await expect(panel.getByRole('alert')).toHaveCount(0);
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.error =
          'Сохранённые режимы устройства'),
    );
    await panel.getByRole('button', { name: copy[language].read, exact: true }).click();
    await expect(panel.getByRole('alert')).toContainText('Сохранённые режимы устройства');
    await expect(panel.getByRole('status')).toHaveCount(0);
  });

  test(`${language}: device-mode controls reflow without translating user names`, async ({
    page,
  }) => {
    await setup(page, language);
    const c = copy[language],
      panel = await open(page, language);
    await panel.getByRole('combobox', { name: c.mode, exact: true }).selectOption('P0');
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    await expect(panel.getByRole('status')).toContainText('Настройки');
    for (const [width, height] of [
      [1280, 1000],
      [1024, 900],
    ]) {
      await page.setViewportSize({ width, height });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
      await page.screenshot({
        path: `../.impeccable/review/cooling-modes-${language}-${width}.png`,
      });
    }
  });
}

for (const bad of [
  'version',
  'missing-channel',
  'duplicate',
  'count',
  'cross-target',
  'fractional',
  'rpm',
  'provenance',
  'default-id',
  'default-name',
  'default-flag',
]) {
  test(`malformed ${bad} response cannot replace a draft or stay selectable`, async ({ page }) => {
    await setup(page, 'en-US');
    await page.getByRole('spinbutton', { name: 'PWM, %', exact: true }).fill('77');
    const panel = await open(page, 'en-US');
    await expect(panel.getByRole('combobox', { name: 'Saved mode', exact: true })).toBeVisible();
    await page.evaluate((bad) => {
      type Entry = { id: string; name: string; policy: Record<string, unknown> };
      type Bank = { version: number; fan: Entry[]; pump: Entry[] };
      const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture,
        root = f.payload.A as Bank;
      let value: unknown = root;
      switch (bad) {
        case 'version':
          value = { ...root, version: 2 };
          break;
        case 'missing-channel':
          value = { ...root, fan: null };
          break;
        case 'duplicate':
          value = { ...root, fan: [root.fan[0], root.fan[0]] };
          break;
        case 'count':
          value = {
            ...root,
            fan: Array.from({ length: 17 }, (_, i) => ({ ...root.fan[0], id: `F${i}` })),
          };
          break;
        case 'cross-target':
          value = { ...root, fan: [{ ...root.fan[0], policy: { mode: 'fixed_rpm', rpm: 2400 } }] };
          break;
        case 'fractional':
          value = {
            ...root,
            fan: [
              {
                ...root.fan[0],
                policy: {
                  ...root.fan[0].policy,
                  points: [
                    { celsius: 25.5, percent: 20 },
                    { celsius: 70, percent: 100 },
                  ],
                },
              },
            ],
          };
          break;
        case 'rpm':
          value = { ...root, pump: [{ ...root.pump[0], policy: { mode: 'fixed_rpm', rpm: 0 } }] };
          break;
        case 'default-id':
          value = { ...root, pump: [{ ...root.pump[0], use_default_name: true }] };
          break;
        case 'default-name':
          value = {
            ...root,
            pump: [
              { ...root.pump[0], id: 'FixRPM', name: 'My literal name', use_default_name: true },
            ],
          };
          break;
        case 'default-flag':
          value = { ...root, pump: [{ ...root.pump[0], use_default_name: 'true' }] };
          break;
        case 'provenance':
          value = { ...root, source: { private: 'must not enter the mode list' } };
          break;
      }
      f.payload.A = value;
    }, bad);
    await panel.getByRole('button', { name: 'Read device modes', exact: true }).click();
    await expect(panel.getByRole('alert')).toContainText(copy['en-US'].invalid);
    await expect(panel.getByRole('combobox', { name: 'Saved mode', exact: true })).toHaveCount(0);
    await expect(page.getByRole('spinbutton', { name: 'PWM, %', exact: true })).toHaveValue('77');
    expect(await writes(page)).toHaveLength(0);
  });
}

for (const language of ['en-US', 'ru-RU'] as const) {
  const name = language === 'en-US' ? 'Mode name' : 'Имя режима';
  const save = language === 'en-US' ? 'Save mode snapshot' : 'Сохранить снимок режима';
  async function editing(page: Page) {
    await setup(page, language);
    const panel = await open(page, language),
      c = copy[language];
    await panel.getByRole('combobox', { name: c.mode, exact: true }).selectOption('P0');
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    return panel;
  }
  test(`${language}: lost save acknowledgement reads back one stored snapshot without Apply`, async ({
    page,
  }) => {
    const panel = await editing(page);
    await panel.getByRole('textbox', { name, exact: true }).fill('My saved pump');
    await page
      .getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      })
      .fill('2600');
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.saveFault =
          'lost'),
    );
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.getByRole('status')).toContainText('My saved pump');
    await expect(panel.getByRole('alert')).toHaveCount(0);
    expect(await writes(page)).toHaveLength(0);
    const calls = await page.evaluate(() =>
      (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
        (c) => c.method === 'save_cooling_mode',
      ),
    );
    expect(calls).toHaveLength(1);
    expect(calls[0].params).toMatchObject({
      dev: 'A',
      target: 'pump',
      expected_revision: '1'.repeat(64),
      profile: { id: 'P0', name: 'My saved pump', policy: { mode: 'fixed_rpm', rpm: 2600 } },
    });
    await page
      .getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      })
      .fill('2700');
    await expect(panel.getByRole('status')).toContainText(
      language === 'en-US' ? 'further edits' : 'другие правки',
    );
    await page
      .getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      })
      .fill('2600');
    await page.getByRole('button', { name: copy[language].apply, exact: true }).click();
    await expect(
      page.getByText(
        language === 'en-US' ? 'Requested device mode: P0' : 'Запрошенный режим устройства: P0',
        { exact: true },
      ),
    ).toBeVisible();
    let applied = await writes(page);
    expect(applied).toHaveLength(1);
    expect(applied[0].method).toBe('apply_cooling_mode');
    await page
      .getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      })
      .fill('2700');
    await page.getByRole('button', { name: copy[language].apply, exact: true }).click();
    await expect(
      page.getByText(
        language === 'en-US' ? 'Requested device mode: P0' : 'Запрошенный режим устройства: P0',
        { exact: true },
      ),
    ).toHaveCount(0);
    applied = await writes(page);
    expect(applied).toHaveLength(2);
    expect(applied[1]).toMatchObject({
      method: 'set_cooling_policy',
      params: { policy: { mode: 'fixed_rpm', rpm: 2700 } },
    });
  });
  test(`${language}: conflict requires explicit read and clearly named overwrite`, async ({
    page,
  }) => {
    const panel = await editing(page),
      c = copy[language];
    await panel.getByRole('textbox', { name, exact: true }).fill('My draft');
    await page
      .getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      })
      .fill('2800');
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.saveFault =
          'conflict'),
    );
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.getByRole('alert')).toBeVisible();
    const overwrite = panel.getByRole('button', {
      name:
        language === 'en-US'
          ? 'Overwrite updated mode with draft'
          : 'Сохранить черновик поверх обновлённого режима',
      exact: true,
    });
    await expect(panel.locator('.edit button')).toBeDisabled();
    await expect(panel.getByRole('button', { name: c.read, exact: true })).toBeFocused();
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.saveFault =
          undefined),
    );
    await panel.getByRole('button', { name: c.read, exact: true }).click();
    await expect(overwrite).toBeEnabled();
    await expect(panel.getByRole('textbox', { name, exact: true })).toHaveValue('My draft');
    await expect(
      page.getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      }),
    ).toHaveValue('2800');
    await overwrite.click();
    await expect(panel.getByRole('alert')).toHaveCount(0);
    await expect(panel.getByRole('status')).toContainText('My draft');
    expect(await writes(page)).toHaveLength(0);
  });
  test(`${language}: pending disk write cannot be reported as saved`, async ({ page }) => {
    const panel = await editing(page);
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.saveFault =
          'pending'),
    );
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.getByRole('alert')).toContainText(
      language === 'en-US' ? 'still being saved' : 'ещё сохраняются',
    );
    await expect(panel.locator('.edit button')).toBeDisabled();
    await page.evaluate(() => {
      const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
      (f.payload.A as { persistence: { pending: boolean } }).persistence.pending = false;
    });
    await panel.getByRole('button', { name: copy[language].read, exact: true }).click();
    await expect(panel.getByRole('alert')).toHaveCount(0);
    await expect(panel.getByRole('status')).toContainText('Настройки');
    expect(await writes(page)).toHaveLength(0);
  });
  test(`${language}: delayed Save preserves later draft changes`, async ({ page }) => {
    const panel = await editing(page);
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.holdSave =
          true),
    );
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(
      panel.getByRole('button', {
        name: language === 'en-US' ? 'Saving…' : 'Сохранение…',
        exact: true,
      }),
    ).toHaveAttribute('aria-busy', 'true');
    await page
      .getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      })
      .fill('2900');
    await page.evaluate(() =>
      (
        window as unknown as { coolingModesFixture: Fixture }
      ).coolingModesFixture.releases.shift()?.(),
    );
    await expect(panel.getByRole('status')).toContainText(
      language === 'en-US' ? 'further edits' : 'другие правки',
    );
    await expect(
      page.getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      }),
    ).toHaveValue('2900');
    expect(await writes(page)).toHaveLength(0);
  });
  test(`${language}: disconnect keeps unresolved Save until explicit read, without replay`, async ({
    page,
  }) => {
    const panel = await editing(page),
      c = copy[language];
    await panel.getByRole('textbox', { name, exact: true }).fill('Reconnect snapshot');
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.holdSave =
          true),
    );
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.locator('.edit button')).toHaveAttribute('aria-busy', 'true');
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.online =
          false),
    );
    await expect(panel.getByRole('button', { name: c.read, exact: true })).toBeDisabled();
    await expect(panel.getByRole('alert')).toContainText(
      language === 'en-US' ? 'unconfirmed' : 'не подтверждён',
    );
    await page.evaluate(() => {
      const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
      f.releases.shift()?.();
      f.online = true;
    });
    await expect(panel.getByRole('button', { name: c.read, exact: true })).toBeEnabled();
    await expect(panel.getByRole('alert')).toContainText(
      language === 'en-US' ? 'unconfirmed' : 'не подтверждён',
    );
    await panel.getByRole('button', { name: c.read, exact: true }).click();
    await expect(panel.getByRole('status')).toContainText('Reconnect snapshot');
    await expect(panel.getByRole('textbox', { name, exact: true })).toHaveValue(
      'Reconnect snapshot',
    );
    expect(
      await page.evaluate(() =>
        (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
          (c) => c.method === 'save_cooling_mode',
        ),
      ),
    ).toHaveLength(1);
    expect(await writes(page)).toHaveLength(0);
  });
  test(`${language}: invalid names never reach storage`, async ({ page }) => {
    const panel = await editing(page);
    for (const invalid of ['  ', 'x'.repeat(129)]) {
      await panel.getByRole('textbox', { name, exact: true }).fill(invalid);
      await panel.getByRole('button', { name: save, exact: true }).click();
      await expect(panel.getByRole('alert')).toBeVisible();
    }
    expect(
      await page.evaluate(() =>
        (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
          (c) => c.method === 'save_cooling_mode',
        ),
      ),
    ).toHaveLength(0);
    expect(await writes(page)).toHaveLength(0);
  });

  test(`${language}: stale stored Apply rejects without falling back to a custom policy`, async ({
    page,
  }) => {
    const panel = await editing(page),
      c = copy[language];
    await page.evaluate(() => {
      const bank = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture
        .payload.A as { revision: string; pump: { policy: { rpm: number } }[] };
      bank.revision = 'b'.repeat(64);
      bank.pump[0].policy.rpm = 2700;
    });
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    await expect(page.getByRole('alert')).toContainText('cooling modes changed');
    const calls = await writes(page);
    expect(calls).toHaveLength(1);
    expect(calls[0].method).toBe('apply_cooling_mode');
    await expect(
      page.getByRole('spinbutton', {
        name:
          language === 'en-US'
            ? 'Target RPM (reference mapping)'
            : 'Целевые RPM (reference mapping)',
        exact: true,
      }),
    ).toHaveValue('2400');
    await panel.getByRole('button', { name: c.read, exact: true }).click();
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    await expect(
      page.getByText(
        language === 'en-US' ? 'Requested device mode: P0' : 'Запрошенный режим устройства: P0',
        { exact: true },
      ),
    ).toBeVisible();
    expect((await writes(page)).every((c) => c.method === 'apply_cooling_mode')).toBe(true);
  });
  test(`${language}: Save completion cannot relabel a subsequently loaded equal policy`, async ({
    page,
  }) => {
    const panel = await editing(page),
      c = copy[language];
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    await expect(
      page.getByText(
        language === 'en-US' ? 'Requested device mode: P0' : 'Запрошенный режим устройства: P0',
        { exact: true },
      ),
    ).toBeVisible();
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.holdSave =
          true),
    );
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.locator('.edit button')).toHaveAttribute('aria-busy', 'true');
    await page
      .getByRole('button', {
        name: language === 'en-US' ? 'Load current policy' : 'Загрузить текущую политику',
        exact: true,
      })
      .click();
    await page.evaluate(() =>
      (
        window as unknown as { coolingModesFixture: Fixture }
      ).coolingModesFixture.releases.shift()?.(),
    );
    await expect(panel.getByRole('status')).toContainText('Настройки');
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    const calls = await writes(page);
    expect(calls).toHaveLength(2);
    expect(calls[1].method).toBe('set_cooling_policy');
  });

  async function emptyBank(page: Page) {
    await setup(page, language);
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.payload.A =
          null),
    );
    return open(page, language);
  }
  const create = language === 'en-US' ? 'Create standard modes' : 'Создать стандартные режимы';
  const source =
    language === 'en-US' ? 'Source for standard modes' : 'Источник стандартных режимов';
  test(`${language}: initialize empty banks without Apply and preserve explicit localized versus literal names`, async ({
    page,
  }) => {
    const panel = await emptyBank(page),
      c = copy[language];
    await page.getByRole('spinbutton', { name: 'PWM, %', exact: true }).fill('77');
    await expect(panel.getByRole('button', { name: create, exact: true })).toBeDisabled();
    for (const [width, height] of process.env.LLCONTROL_SKIP_DESIGN_CAPTURE
      ? []
      : [
          [1280, 1000],
          [1024, 900],
        ]) {
      await page.setViewportSize({ width, height });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
      await page.screenshot({
        path: `../.impeccable/review/cooling-mode-defaults-empty-${language}-${width}.png`,
      });
    }
    await panel.getByRole('combobox', { name: source, exact: true }).selectOption('cpu:test');
    await panel.getByRole('button', { name: create, exact: true }).click();
    await expect(
      panel.getByText(
        language === 'en-US'
          ? 'Standard modes were saved. Running settings are unchanged; choose a mode and load it into the draft.'
          : 'Стандартные режимы сохранены. Работающие настройки не изменены; выберите режим и загрузите его в черновик.',
        { exact: true },
      ),
    ).toBeVisible();
    await expect(page.getByRole('spinbutton', { name: 'PWM, %', exact: true })).toHaveValue('77');
    expect(await writes(page)).toHaveLength(0);
    const select = panel.getByRole('combobox', { name: c.mode, exact: true });
    await expect(select.locator('option')).toHaveCount(4);
    await expect(select).toBeFocused();
    await select.selectOption('FixRPM');
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    await expect(panel.getByRole('textbox', { name, exact: true })).toHaveValue(
      language === 'en-US' ? 'Fixed RPM' : 'Фиксированные RPM',
    );
    for (const [width, height] of process.env.LLCONTROL_SKIP_DESIGN_CAPTURE
      ? []
      : [
          [1280, 1000],
          [1024, 900],
        ]) {
      await page.setViewportSize({ width, height });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
      await page.screenshot({
        path: `../.impeccable/review/cooling-mode-defaults-loaded-${language}-${width}.png`,
      });
    }

    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.getByRole('status')).toContainText(
      language === 'en-US' ? 'Fixed RPM' : 'Фиксированные RPM',
    );
    const calls = await page.evaluate(() =>
      (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
        (c) => c.method === 'save_cooling_mode',
      ),
    );
    expect(calls[0].params.profile).toMatchObject({
      id: 'FixRPM',
      name: 'Fixed RPM',
      use_default_name: true,
      policy: { mode: 'fixed_rpm', rpm: 2200 },
    });
    await panel.getByRole('textbox', { name, exact: true }).fill('Quiet');
    await panel.getByRole('button', { name: save, exact: true }).click();
    for (const next of ['ru-RU', 'en-US', 'ru-RU'] as const) {
      await page.evaluate(async (language: 'en-US' | 'ru-RU') => {
        const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
        f.settings.interface_language = language;
        f.settings.revision++;
        const path = '/src/lib/presentation.svelte.ts';
        const { publishDisplaySettings } = await import(/* @vite-ignore */ path);
        publishDisplaySettings(f.settings);
      }, next);
      await expect(
        panel
          .getByRole('combobox', { name: copy[next].mode, exact: true })
          .locator('option[value="PWMQuiet"]'),
      ).toHaveText(next === 'en-US' ? 'Quiet (PWMQuiet)' : 'Тихий (PWMQuiet)');
      await expect(
        panel
          .getByRole('combobox', { name: copy[next].mode, exact: true })
          .locator('option[value="FixRPM"]'),
      ).toHaveText('Quiet (FixRPM)');
    }
    expect(await writes(page)).toHaveLength(0);
    const initializers = await page.evaluate(() =>
      (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
        (c) => c.method === 'initialize_cooling_modes',
      ),
    );
    expect(initializers).toHaveLength(1);
    expect(initializers[0].params).toEqual({ dev: 'A', sensor: 'cpu:test' });
  });
  for (const fault of ['lost', 'pending'] as const) {
    test(`${language}: ${fault} initialization never blindly replays or claims pending data saved`, async ({
      page,
    }) => {
      const panel = await emptyBank(page),
        c = copy[language];
      await page.evaluate(
        (fault) =>
          ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.saveFault =
            fault),
        fault,
      );
      await panel.getByRole('combobox', { name: source, exact: true }).selectOption('cpu:test');
      await panel.getByRole('button', { name: create, exact: true }).click();
      if (fault === 'pending') {
        await expect(panel.getByRole('alert')).toContainText(
          language === 'en-US' ? 'still being saved' : 'ещё сохраняются',
        );
        await expect(panel.getByRole('combobox', { name: c.mode, exact: true })).toBeDisabled();
        await expect(panel.getByRole('button', { name: c.read, exact: true })).toBeFocused();
        await page.evaluate(() => {
          const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
          (f.payload.A as { persistence: { pending: boolean } }).persistence.pending = false;
        });
        await panel.getByRole('button', { name: c.read, exact: true }).click();
      }
      await expect(
        panel.getByText(
          language === 'en-US'
            ? 'Stored modes were found. Review the list before loading; no second write was sent.'
            : 'Найдены сохранённые режимы. Проверьте список перед загрузкой; повторная запись не отправлялась.',
          { exact: true },
        ),
      ).toBeVisible();
      expect(
        await page.evaluate(() =>
          (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
            (c) => c.method === 'initialize_cooling_modes',
          ),
        ),
      ).toHaveLength(1);
      expect(await writes(page)).toHaveLength(0);
    });
  }

  test(`${language}: initialization completion preserves focus in an edited policy`, async ({
    page,
  }) => {
    const panel = await emptyBank(page);
    await page.evaluate(
      () =>
        ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.holdSave =
          true),
    );
    await panel.getByRole('combobox', { name: source, exact: true }).selectOption('cpu:test');
    await panel.getByRole('button', { name: create, exact: true }).click();
    await expect(
      panel.getByRole('button', {
        name: language === 'en-US' ? 'Creating…' : 'Создание…',
        exact: true,
      }),
    ).toHaveAttribute('aria-busy', 'true');
    const pwm = page.getByRole('spinbutton', { name: 'PWM, %', exact: true });
    await pwm.fill('88');
    await page.evaluate(() =>
      (
        window as unknown as { coolingModesFixture: Fixture }
      ).coolingModesFixture.releases.shift()?.(),
    );
    await expect(
      panel.getByRole('combobox', { name: copy[language].mode, exact: true }),
    ).toBeVisible();
    await expect(pwm).toBeFocused();
    await expect(pwm).toHaveValue('88');
    expect(await writes(page)).toHaveLength(0);
  });

  test(`${language}: explicitly typed factory label becomes a literal name`, async ({ page }) => {
    const panel = await emptyBank(page),
      c = copy[language];
    await panel.getByRole('combobox', { name: source, exact: true }).selectOption('cpu:test');
    await panel.getByRole('button', { name: create, exact: true }).click();
    await panel.getByRole('combobox', { name: c.mode, exact: true }).selectOption('PWMQuiet');
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    const literal = language === 'en-US' ? 'Quiet' : 'Тихий';
    await panel.getByRole('textbox', { name, exact: true }).fill(literal);
    await panel.getByRole('button', { name: save, exact: true }).click();
    const saved = await page.evaluate(() =>
      (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
        (c) => c.method === 'save_cooling_mode',
      ),
    );
    expect(saved).toHaveLength(1);
    expect(saved[0].params.profile).toMatchObject({ id: 'PWMQuiet', name: literal });
    expect(saved[0].params.profile).not.toHaveProperty('use_default_name');
    const next: 'en-US' | 'ru-RU' = language === 'en-US' ? 'ru-RU' : 'en-US';
    await page.evaluate(async (language: 'en-US' | 'ru-RU') => {
      const f = (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture;
      f.settings.interface_language = language;
      f.settings.revision++;
      const path = '/src/lib/presentation.svelte.ts';
      const { publishDisplaySettings } = await import(/* @vite-ignore */ path);
      publishDisplaySettings(f.settings);
    }, next);
    await expect(
      panel
        .getByRole('combobox', { name: copy[next].mode, exact: true })
        .locator('option[value="PWMQuiet"]'),
    ).toHaveText(`${literal} (PWMQuiet)`);
    expect(await writes(page)).toHaveLength(0);
  });

  const resetName = language === 'en-US' ? 'Restore default name' : 'Вернуть стандартное имя';
  const resetMode =
    language === 'en-US' ? 'Load factory settings into draft' : 'Стандартные параметры в черновик';
  const rpmLabel =
    language === 'en-US' ? 'Target RPM (reference mapping)' : 'Целевые RPM (reference mapping)';
  async function factoryEditing(page: Page, id = 'FixRPM') {
    const panel = await emptyBank(page),
      c = copy[language];
    await panel.getByRole('combobox', { name: source, exact: true }).selectOption('cpu:test');
    await panel.getByRole('button', { name: create, exact: true }).click();
    await panel.getByRole('combobox', { name: c.mode, exact: true }).selectOption(id);
    await panel.getByRole('button', { name: c.load, exact: true }).click();
    return panel;
  }
  test(`${language}: per-mode/name restore stays local, then explicit Save and Apply choose separate intents`, async ({
    page,
  }) => {
    const panel = await factoryEditing(page),
      c = copy[language],
      rpm = page.getByRole('spinbutton', { name: rpmLabel, exact: true });
    await panel.getByRole('textbox', { name, exact: true }).fill('Renamed pump');
    await rpm.fill('2800');
    await panel.getByRole('button', { name: save, exact: true }).click();
    await expect(panel.getByRole('status')).toContainText('Renamed pump');
    const bankBefore = await page.evaluate(
      () => (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.payload.A,
    );
    await panel.getByRole('button', { name: resetName, exact: true }).click();
    await expect(rpm).toHaveValue('2800');
    expect(
      await page.evaluate(
        () => (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.payload.A,
      ),
    ).toEqual(bankBefore);
    await panel.getByRole('button', { name: resetMode, exact: true }).click();
    await expect(rpm).toHaveValue('2200');
    await expect(panel.getByRole('textbox', { name, exact: true })).toHaveValue(
      language === 'en-US' ? 'Fixed RPM' : 'Фиксированные RPM',
    );
    expect(
      await page.evaluate(
        () => (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.payload.A,
      ),
    ).toEqual(bankBefore);
    expect(await writes(page)).toHaveLength(0);
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    let calls = await writes(page);
    expect(calls[0]).toMatchObject({
      method: 'set_cooling_policy',
      params: { policy: { mode: 'fixed_rpm', rpm: 2200 } },
    });
    await panel.getByRole('button', { name: save, exact: true }).click();
    await page.getByRole('button', { name: c.apply, exact: true }).click();
    calls = await writes(page);
    expect(calls[1].method).toBe('apply_cooling_mode');
    for (const [width, height] of [
      [1280, 1000],
      [1024, 900],
    ]) {
      await page.setViewportSize({ width, height });
      await panel.scrollIntoViewIfNeeded();
      expect(await panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
      await page.screenshot({
        path: `../.impeccable/review/cooling-mode-reset-${language}-${width}.png`,
      });
    }
  });
  test(`${language}: factory curve reset requires a source and recovers an invalid local draft`, async ({
    page,
  }) => {
    const panel = await factoryEditing(page, 'PWMQuiet');
    const sensor = page.getByRole('combobox', {
      name: language === 'en-US' ? 'Temperature source' : 'Источник температуры',
      exact: true,
    });
    await sensor.selectOption('');
    await expect(panel.getByRole('button', { name: resetMode, exact: true })).toBeDisabled();
    await sensor.selectOption('cpu:test');
    const point = page.getByRole('spinbutton', { name: 'RPM', exact: true }).first();
    await point.fill('7000');
    await panel.getByRole('button', { name: resetMode, exact: true }).click();
    await expect(point).toHaveValue('2000');
    expect(await writes(page)).toHaveLength(0);
    const prepare = await page.evaluate(() =>
      (window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.calls.filter(
        (c) => c.method === 'cooling_mode_default',
      ),
    );
    expect(prepare).toHaveLength(1);
    expect(prepare[0].params).toEqual({ target: 'pump', id: 'PWMQuiet', sensorId: 'cpu:test' });
  });
  for (const change of ['draft', 'device'] as const) {
    test(`${language}: late factory preparation preserves ${change} context`, async ({ page }) => {
      const panel = await factoryEditing(page);
      await page.evaluate(
        () =>
          ((window as unknown as { coolingModesFixture: Fixture }).coolingModesFixture.holdDefault =
            true),
      );
      await panel.getByRole('button', { name: resetMode, exact: true }).click();
      await expect(
        panel.getByRole('button', {
          name: language === 'en-US' ? 'Preparing…' : 'Подготовка…',
          exact: true,
        }),
      ).toHaveAttribute('aria-busy', 'true');
      if (change === 'draft') {
        await page.getByRole('spinbutton', { name: rpmLabel, exact: true }).fill('2700');
        await panel.getByRole('textbox', { name, exact: true }).fill('Keep my name');
      } else {
        await page
          .getByRole('combobox', {
            name: language === 'en-US' ? 'Device' : 'Устройство',
            exact: true,
          })
          .selectOption('B');
        await panel.getByRole('button', { name: copy[language].read, exact: true }).click();
        await panel
          .getByRole('combobox', { name: copy[language].mode, exact: true })
          .selectOption('P0');
        await panel.getByRole('button', { name: copy[language].load, exact: true }).click();
      }
      await page.evaluate(() =>
        (
          window as unknown as { coolingModesFixture: Fixture }
        ).coolingModesFixture.releases.shift()?.(),
      );
      await expect(page.getByRole('spinbutton', { name: rpmLabel, exact: true })).toHaveValue(
        change === 'draft' ? '2700' : '2400',
      );
      await expect(panel.getByRole('textbox', { name, exact: true })).toHaveValue(
        change === 'draft' ? 'Keep my name' : 'B Настройки',
      );
      expect(await writes(page)).toHaveLength(0);
    });
  }
  for (const fault of ['invalid', 'error'] as const) {
    test(`${language}: ${fault} factory response does not change draft or stored mode`, async ({
      page,
    }) => {
      const panel = await factoryEditing(page);
      await page.getByRole('spinbutton', { name: rpmLabel, exact: true }).fill('2800');
      await page.evaluate(
        (fault) =>
          ((
            window as unknown as { coolingModesFixture: Fixture }
          ).coolingModesFixture.defaultFault = fault),
        fault,
      );
      await panel.getByRole('button', { name: resetMode, exact: true }).click();
      await expect(panel.getByRole('alert')).toBeVisible();
      await expect(page.getByRole('spinbutton', { name: rpmLabel, exact: true })).toHaveValue(
        '2800',
      );
      expect(await writes(page)).toHaveLength(0);
    });
  }
}
