(async () => {
  if (window.__nativeBasicEditorsStarted) return;
  window.__nativeBasicEditorsStarted = true;
  const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args),
    rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    assert = (value, why) => {
      if (!value) throw new Error(why);
    };
  const until = async (check, why) => {
    for (let i = 0; i < 200; i++) {
      if (await check()) return;
      await delay(50);
    }
    throw new Error('timeout: ' + why);
  };
  const english = window.__nativeDesktopLanguage === 'en-US',
    t = (en, ru) => (english ? en : ru);
  const button = (text) =>
    Array.from(document.querySelectorAll('button')).find(
      (node) => node.textContent.trim() === text,
    );
  const control = (text) =>
    Array.from(document.querySelectorAll('label'))
      .find(
        (node) =>
          Array.from(node.childNodes)
            .filter((child) => child.nodeType === Node.TEXT_NODE)
            .map((child) => child.textContent)
            .join('')
            .trim() === text,
      )
      ?.querySelector('input,select,textarea');
  const click = async (text) => {
    await until(() => button(text) && !button(text).disabled, text);
    button(text).click();
    await delay(30);
  };
  const set = async (text, value) => {
    await until(() => control(text) && !control(text).disabled, text);
    const node = control(text);
    node.value = value;
    node.dispatchEvent(
      new Event(node.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }),
    );
    await delay(40);
  };
  const textInput = () =>
    document.querySelector('[data-studio-panel="composition"] .layer textarea');
  const writeText = async (value) => {
    await until(textInput, 'custom text');
    const node = textInput();
    node.value = value;
    node.dispatchEvent(new Event('input', { bubbles: true }));
    await delay(30);
  };
  const background = t('Background preset', 'Вариант фона'),
    overlay = t('Text preset', 'Вариант текста');
  try {
    let devices = [];
    await until(async () => {
      devices = await rpc('list_devices');
      return devices.length === 1;
    }, 'mock device');
    const dev = devices[0].id;
    const active = await rpc('get_lcd_theme', { dev }),
      recovery = active?.name === 'Native authored banks';
    await click(t('Pump screen', 'Экран помпы'));
    if (recovery) {
      await click(t('Load active theme', 'Загрузить активную тему'));
      await until(() => control(background), 'restarted banks');
      assert(textInput()?.value === 'Native image authored', 'restart active custom text');
      await set(background, 'background_color');
      assert(textInput()?.value === 'Native color authored', 'restart inactive custom text');
      assert(
        control(t('First background color', 'Первый цвет фона')).value === '#a1b2c3',
        'restart inactive shared gradient',
      );
      const persisted = await rpc('get_lcd_theme', { dev });
      assert(
        JSON.stringify(persisted) === JSON.stringify(active),
        'draft restart switching must not apply',
      );
      await invoke('native_smoke_result', {
        keepOpen: true,
        result: {
          ok: true,
          recovery: true,
          dev,
          theme: active,
          library: await rpc('list_profiles'),
        },
      });
      return;
    }
    assert(!active, 'initial fixture has no applied theme');
    await until(
      () =>
        Array.from(document.querySelectorAll('summary')).some(
          (node) => node.textContent.trim() === t('Saved profiles', 'Сохранённые профили'),
        ),
      'profiles',
    );
    Array.from(document.querySelectorAll('summary'))
      .find((node) => node.textContent.trim() === t('Saved profiles', 'Сохранённые профили'))
      .click();
    await click(t('Refresh profile list', 'Обновить список профилей'));
    await click(t('Load profile into editor', 'Загрузить профиль в редактор'));
    await until(() => control(background), 'loaded banks');
    await set(t('Name', 'Название'), 'Native authored banks');
    await writeText('Native image authored');
    await set(background, 'background_color');
    await writeText('Native color authored');
    await set(t('First background color', 'Первый цвет фона'), '#a1b2c3');
    let previews = 0;
    for (const bank of ['background_color', 'image_video']) {
      await set(background, bank);
      for (const kind of [
        'none',
        'slider_sensor',
        'duo_sensor',
        'three_sensor',
        'four_sensor',
        'time',
        'custom',
      ]) {
        await set(overlay, kind);
        if (bank === 'background_color')
          assert(
            control(t('First background color', 'Первый цвет фона')).value === '#a1b2c3',
            'shared gradient survives text preset',
          );
        await click(t('Refresh preview', 'Обновить preview'));
        await until(
          () =>
            Array.from(document.querySelectorAll('[role=status]')).some((node) =>
              node.textContent.includes(
                t(
                  'Preview updated. Nothing was sent to the device.',
                  'Preview обновлён. На устройство ничего не отправлено.',
                ),
              ),
            ),
          'native preview status',
        );
        assert((await rpc('get_lcd_theme', { dev })) === null, 'preview/selection must not apply');
        previews++;
      }
    }
    assert(textInput()?.value === 'Native image authored', 'image custom edit survived');
    assert(
      control(t('Name', 'Название')).value === 'Native authored banks',
      'preset does not replace theme name',
    );
    await click(t('+ Divider', '+ Разделитель'));
    const divider = document.querySelector('.divider-controls');
    assert(divider, 'native divider controls');
    const local = (label) =>
      [...divider.querySelectorAll('label')]
        .find(
          (node) =>
            [...node.childNodes]
              .filter((n) => n.nodeType === Node.TEXT_NODE)
              .map((n) => n.textContent)
              .join('')
              .trim() === label,
        )
        ?.querySelector('input');
    for (const [label, value] of [
      ['X', '34.5'],
      ['Y', '430'],
      [t('Length, px', 'Длина, px'), '400'],
      [t('Color', 'Цвет'), '#12f032'],
    ]) {
      const input = local(label);
      assert(input, label);
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
    const shadow = local(t('Divider shadow', 'Тень разделителя'));
    shadow.checked = false;
    shadow.dispatchEvent(new Event('change', { bubbles: true }));
    await delay(50);
    await click(t('Replace profile with draft', 'Заменить профиль черновиком'));
    await until(async () => (await rpc('list_profiles')).revision === 2, 'saved draft');
    assert((await rpc('get_lcd_theme', { dev })) === null, 'Save must not apply');
    await click(t('Apply to LCD', 'Применить к LCD'));
    await until(
      async () => (await rpc('get_lcd_theme', { dev }))?.name === 'Native authored banks',
      'native Apply',
    );
    const theme = await rpc('get_lcd_theme', { dev }),
      library = await rpc('list_profiles');
    assert(
      theme.layers.some((layer) => layer.kind === 'divider' && layer.x === 34.5 && layer.y === 430),
      'divider stored in active scene',
    );
    assert(
      JSON.stringify(theme) === JSON.stringify(library.profiles[0].content.value),
      'Apply matches saved banks',
    );
    const color = theme.basic_editor.banks.find((bank) => bank.background === 'background_color');
    assert(
      color.overlays.every(
        (item) =>
          JSON.stringify(item.theme.background_fill.horizontal.colors[0]) === '[161,178,195]',
      ),
      'all color presets share edited background',
    );
    await invoke('native_smoke_result', {
      keepOpen: true,
      result: {
        ok: true,
        recovery: false,
        dev,
        theme,
        library,
        previews,
        divider_edit: true,
        explicit_apply: true,
        save_does_not_apply: true,
      },
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      result: {
        ok: false,
        error: error?.message ?? String(error),
        stack: error?.stack,
        body: document.body.innerText,
      },
    });
  }
})();
