(async () => {
  if (window.__nativeKeyboardStarted) return;
  window.__nativeKeyboardStarted = true;
  const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const assert = (value, reason) => {
    if (!value) throw new Error(reason);
  };
  const wait = async (predicate, reason) => {
    for (let i = 0; i < 100; i++) {
      if (await predicate()) return;
      await delay(50);
    }
    throw new Error(reason);
  };
  const button = (text) =>
    [...document.querySelectorAll('button')].find((el) => el.textContent.trim() === text);
  const field = (text) =>
    [...document.querySelectorAll('label')]
      .find((el) => el.textContent.trim().startsWith(text))
      ?.querySelector('input,select');
  let sequence = 0;
  let lastTabTarget;
  document.addEventListener(
    'keydown',
    (event) => {
      if (event.isTrusted && event.key === 'Tab') lastTabTarget = event.target;
    },
    true,
  );
  const observations = [];
  const reflow = [];
  let importedImageAssets = [];
  function inspectReflow(page) {
    const viewport = document.documentElement.clientWidth;
    const width = document.documentElement.scrollWidth;
    assert(width <= viewport + 1, `${page}: horizontal overflow ${width}/${viewport}`);
    reflow.push({
      page,
      viewport,
      width,
      zoom: window.__nativeZoom,
      dpr: devicePixelRatio,
      bodyFont: getComputedStyle(document.body).fontSize,
      rootFont: getComputedStyle(document.documentElement).fontSize,
    });
  }
  const windowFocusEvents = [];
  for (const type of ['blur', 'focus'])
    window.addEventListener(type, (event) => {
      if (event.isTrusted) windowFocusEvents.push(type);
    });
  async function key(key, probe = null) {
    let listener, timer;
    const received = new Promise((resolve, reject) => {
      const expected =
        {
          Return: 'Enter',
          space: ' ',
          Down: 'ArrowDown',
          Up: 'ArrowUp',
          Left: 'ArrowLeft',
          Right: 'ArrowRight',
          'shift+Tab': 'Tab',
        }[key] ?? key;
      listener = (event) => {
        if (event.isTrusted && event.key === expected) resolve();
      };
      document.addEventListener('keyup', listener, true);
      timer = setTimeout(
        () => reject(new Error('No trusted keyup: ' + key)),
        probe?.startsWith('fixture_file_') ? 20000 : 5000,
      );
    });
    try {
      if (['settings', 'cooling', 'lighting', 'sensors', 'display'].includes(probe))
        inspectReflow(probe);
      await invoke('native_keyboard_step', { sequence: ++sequence, key, probe });
      await received;
    } finally {
      clearTimeout(timer);
      document.removeEventListener('keyup', listener, true);
    }
    await delay(50);
  }
  let lastAnchor;
  async function tabTo(element) {
    assert(element && !element.disabled, 'Keyboard target missing/disabled');
    for (let i = 0; i < 200; i++) {
      if (document.activeElement === element) {
        const css = getComputedStyle(element);
        assert(
          css.outlineStyle !== 'none' && parseFloat(css.outlineWidth) >= 2,
          'Invisible focus outline',
        );
        lastAnchor = element;
        return;
      }
      const active = document.activeElement === document.body ? lastAnchor : document.activeElement;
      const backwards =
        active?.isConnected &&
        active.compareDocumentPosition(element) & Node.DOCUMENT_POSITION_PRECEDING;
      await key(backwards ? 'shift+Tab' : 'Tab');
    }
    throw new Error('Tab target unreachable: ' + element.textContent);
  }
  const activate = async (text) => {
    await tabTo(button(text));
    await key('Return');
  };
  try {
    await wait(() => document.body.textContent.includes('Daemon подключён'), 'Daemon connection');
    const devices = await rpc('list_devices');
    assert(devices.length === 1, 'Mock device');
    const dev = devices[0].id;
    await key('Tab');
    assert(
      document.activeElement?.classList.contains('skip-link'),
      'Initial Tab reaches skip link',
    );
    await key('Return');
    assert(document.activeElement?.id === 'main-content', 'Skip link focuses main');
    observations.push('skip-link');

    await activate('Настройки');
    await activate('Загрузить текущие значения');
    const brightness = field('Яркость LCD:');
    await tabTo(brightness);
    const focusEventStart = windowFocusEvents.length;
    await key('End', 'settings'); // Python performs a real Alt+Tab round trip before this key.
    assert(
      windowFocusEvents.slice(focusEventStart).includes('blur') &&
        windowFocusEvents.slice(focusEventStart).includes('focus'),
      'Trusted window blur/focus missing',
    );
    assert(
      document.hasFocus() && document.activeElement === brightness,
      'Brightness focus not restored after Alt+Tab',
    );
    observations.push('window-switch:brightness-focus-restored');
    await key('Left');
    assert(brightness.value === '99', 'Brightness keyboard edit');
    await activate('Применить яркость');
    await wait(
      async () => (await rpc('get_lcd_state', { dev })).requested.brightness_percent === 99,
      'Brightness IPC readback',
    );
    const rotation = field('Ориентация');
    await tabTo(rotation);
    await key('Home');
    await key('Down');
    await key('Tab');
    assert(rotation.value === '90', 'Rotation keyboard selection');
    await activate('Применить поворот');
    await wait(
      async () => (await rpc('get_lcd_state', { dev })).requested.rotation_degrees === 90,
      'Rotation IPC readback',
    );
    observations.push('settings:99/90');

    await activate('Охлаждение');
    const channel = field('Канал');
    await tabTo(channel);
    await key('End');
    await key('Tab');
    assert(channel.value === 'fan', 'Fan keyboard selection');
    const pwm = field('PWM, %');
    await tabTo(pwm);
    await key('Down', 'cooling');
    assert(pwm.value === '79', 'PWM keyboard edit');
    await key('Tab', 'cooling'); // Re-read AT-SPI after the value changed.
    await activate('Применить охлаждение');
    await wait(
      async () => (await rpc('get_cooling_state', { dev })).fan.applied_percent === 79,
      'Fan IPC readback',
    );
    observations.push('cooling:fan79');
    const coolingMode = field('Режим');
    await tabTo(coolingMode);
    await key('End');
    await key('Tab');
    const curveTemperature = field('Точка 1: °C');
    await tabTo(curveTemperature);
    await key('Up');
    assert(curveTemperature.value === '31', 'Curve temperature keyboard edit');
    await key('Tab', 'cooling');
    assert(
      (await rpc('get_cooling_state', { dev })).fan.requested.mode === 'fixed',
      'Curve draft must not apply',
    );
    await tabTo(coolingMode);
    await key('Home');
    await key('Tab');
    observations.push('cooling:curve-numeric-draft');
    await tabTo(channel);
    await key('Home');
    await key('Tab');
    assert(channel.value === 'pump', 'Pump keyboard selection');
    await tabTo(coolingMode);
    await key('Home');
    await key('Down');
    await key('Down');
    await key('Tab');
    assert(coolingMode.value === 'fixed_rpm', 'Fixed RPM mode');
    const rpm = field('Целевые RPM');
    await tabTo(rpm);
    await key('Up');
    assert(rpm.value === '2201', 'RPM keyboard edit');
    await key('Tab', 'cooling');
    await activate('Применить охлаждение');
    await wait(
      async () => (await rpc('get_cooling_state', { dev })).pump.requested?.rpm === 2201,
      'Pump RPM IPC readback',
    );
    assert(
      (await rpc('get_cooling_state', { dev })).pump.applied_percent === 55,
      'RPM mapping applied duty',
    );
    observations.push('cooling:pump-fixed-rpm2201');
    await tabTo(coolingMode);
    await key('Home');
    await key('Down');
    await key('Down');
    await key('Down');
    await key('Tab');
    assert(coolingMode.value === 'curve_rpm', 'Curve RPM mode');
    const pumpTemperature = field('Точка 1: °C');
    await tabTo(pumpTemperature);
    await key('Up');
    await key('Tab', 'cooling');
    const curveRpm = field('RPM');
    await tabTo(curveRpm);
    await key('Up');
    assert(curveRpm.value === '2201', 'Curve RPM keyboard edit');
    const temperatureSource = field('Источник температуры');
    await tabTo(temperatureSource);
    await key('Home');
    await key('Down');
    await key('Tab');
    assert(temperatureSource.value, 'Pump curve sensor chosen');
    assert(
      (await rpc('get_cooling_state', { dev })).pump.requested?.mode === 'fixed_rpm',
      'RPM curve draft must not apply',
    );
    await activate('Применить охлаждение');
    await wait(async () => {
      const pump = (await rpc('get_cooling_state', { dev })).pump;
      return (
        pump.requested?.mode === 'curve_rpm' &&
        pump.requested.points[0].celsius === 31 &&
        pump.requested.points[0].rpm === 2201 &&
        pump.applied_percent !== null &&
        !pump.last_error
      );
    }, 'Pump RPM curve IPC readback');
    observations.push('cooling:pump-curve-rpm2201/31');

    await activate('Подсветка');
    await wait(() => !button('Применить подсветку')?.disabled, 'Lighting modes loaded');
    const rgb = field('Яркость RGB:');
    await tabTo(rgb);
    await key('End', 'lighting');
    await key('Left');
    assert(rgb.value === '99', 'RGB keyboard edit');
    await activate('Применить подсветку');
    await wait(
      async () => (await rpc('get_thermal_state', { dev })).grace_remaining_ms > 0,
      'Manual grace IPC readback',
    );
    observations.push('lighting:manual-grace');
    const zone = field('Зона');
    await tabTo(zone);
    await key('End');
    await key('Tab');
    await wait(() => !button('Применить подсветку')?.disabled, 'Fan modes ready');
    await activate('Применить подсветку');
    await activate('Загрузить текущие оповещения');
    const enableCpu = field('Включить CPU');
    await tabTo(enableCpu);
    await key('space');
    const thermalSensor = field('Датчик CPU');
    await tabTo(thermalSensor);
    await key('Home');
    await key('Down');
    await key('Tab');
    const threshold = field('Порог CPU');
    await tabTo(threshold);
    await key('Up');
    assert(threshold.value === '86', 'Thermal threshold keyboard edit');
    await key('Tab', 'lighting');
    assert(
      (await rpc('get_thermal_state', { dev })).requested.cpu === null,
      'Thermal draft must not apply',
    );
    await activate('Применить оповещения');
    await wait(
      async () => (await rpc('get_thermal_state', { dev })).requested.cpu?.threshold_c === 86,
      'Thermal IPC readback',
    );
    observations.push('lighting:thermal86');
    await tabTo(zone);
    await key('Home');
    await key('Tab');
    const effect = field('Эффект');
    await wait(
      () => [...effect.options].some((option) => option.value === 'breathing'),
      'Pump breathing available',
    );
    await tabTo(effect);
    await key('Home');
    for (
      let i = 0;
      i < [...effect.options].findIndex((option) => option.value === 'breathing');
      i++
    )
      await key('Down');
    await key('Tab');
    assert(effect.value === 'breathing', 'Breathing keyboard selection');
    await activate('Применить подсветку');
    const dynamicDetails = [...document.querySelectorAll('summary')].find(
      (el) => el.textContent.trim() === 'Подсветка помпы по датчику',
    );
    await tabTo(dynamicDetails);
    await key('Return');
    await wait(
      () => !button('Загрузить настройки режима по датчику')?.disabled,
      'Dynamic editor ready',
    );
    await activate('Загрузить настройки режима по датчику');
    const dynamicSensor = field('Датчик подсветки');
    async function chooseDynamicUnit(unit) {
      const index = [...dynamicSensor.options].findIndex((option) =>
        option.textContent.includes(' · ' + unit + ' · '),
      );
      assert(index > 0, 'Fixture source missing: ' + unit);
      await tabTo(dynamicSensor);
      await key('Home');
      for (let i = 0; i < index; i++) await key('Down');
      await key('Tab');
    }
    await chooseDynamicUnit('celsius');
    const low = field('Нижний порог');
    await tabTo(low);
    await key('Up');
    const high = field('Верхний порог');
    await tabTo(high);
    await key('Up');
    const lowSpeed = field('Нижняя скорость');
    await tabTo(lowSpeed);
    for (let n = Number(lowSpeed.value); n > 0; n--) await key('Down');
    await key('Up');
    const highSpeed = field('Верхняя скорость');
    await tabTo(highSpeed);
    for (let n = Number(highSpeed.value); n < 100; n++) await key('Up');
    await key('Down');
    await tabTo(low);
    await key('Tab', 'lighting');
    assert(
      (await rpc('get_dynamic_rgb', { dev })).requested === null,
      'Dynamic draft must not apply',
    );
    await activate('Применить режим по датчику');
    await wait(async () => {
      const state = await rpc('get_dynamic_rgb', { dev });
      return (
        state.requested?.unit === 'celsius' &&
        state.requested.low_value === 31 &&
        state.requested.high_value === 61 &&
        state.requested.low_speed_percent === 1 &&
        state.requested.high_speed_percent === 99 &&
        state.applied !== null &&
        !state.last_error
      );
    }, 'Dynamic celsius IPC readback');
    const beforeInvalid = JSON.stringify((await rpc('get_dynamic_rgb', { dev })).requested);
    await tabTo(high);
    for (let i = 0; i < 31; i++) await key('Down');
    assert(
      high.value === '30' &&
        high.getAttribute('aria-invalid') === 'true' &&
        low.getAttribute('aria-invalid') === 'true',
      'Inverted thresholds must be invalid',
    );
    await activate('Применить режим по датчику');
    await wait(
      () =>
        [...document.querySelectorAll('[role="alert"]')].some((el) =>
          el.textContent.includes('Пороги температуры должны соответствовать целым'),
        ),
      'Dynamic validation message',
    );
    assert(
      JSON.stringify((await rpc('get_dynamic_rgb', { dev })).requested) === beforeInvalid,
      'Invalid draft changed active dynamic config',
    );
    await tabTo(high);
    await key('Tab', 'lighting');
    await tabTo(high);
    for (let i = 0; i < 31; i++) await key('Up');
    assert(
      high.value === '61' &&
        high.getAttribute('aria-invalid') === 'false' &&
        low.getAttribute('aria-invalid') === 'false',
      'Corrected thresholds must recover',
    );
    await activate('Применить режим по датчику');
    await wait(
      () =>
        ![...document.querySelectorAll('[role="alert"]')].some((el) =>
          el.textContent.includes('Пороги температуры должны соответствовать целым'),
        ),
      'Validation error cleared after valid Apply',
    );
    await tabTo(low);
    await key('Tab', 'lighting');
    observations.push('lighting:dynamic-invalid/recovery');
    await chooseDynamicUnit('rpm');
    const rpmLow = field('Нижний порог'),
      rpmHigh = field('Верхний порог');
    await tabTo(rpmLow);
    await key('Up');
    await tabTo(rpmHigh);
    await key('Up');
    await tabTo(rpmLow);
    await key('Tab', 'lighting');
    assert(
      (await rpc('get_dynamic_rgb', { dev })).requested.unit === 'celsius',
      'Unit draft must not apply',
    );
    await activate('Применить режим по датчику');
    await wait(
      async () => (await rpc('get_dynamic_rgb', { dev })).requested?.unit === 'rpm',
      'Dynamic RPM IPC readback',
    );
    await activate('Отключить режим по датчику');
    await wait(
      async () => (await rpc('get_dynamic_rgb', { dev })).requested === null,
      'Dynamic disabled IPC readback',
    );
    observations.push('lighting:dynamic-celsius31-61/speed1-99/rpm801-1201/off');

    await activate('Датчики');
    await wait(() => field('CPU · Температура'), 'Source editor');
    const source = field('CPU · Температура');
    await tabTo(source);
    await key('End', 'sensors');
    await key('Tab');
    const selected = source.value;
    assert(selected, 'CPU source selected');
    await activate('Применить источники');
    await wait(
      async () => (await rpc('get_sensor_defaults')).config.cpu.temperature === selected,
      'Sensor source IPC readback',
    );
    observations.push('sensors:' + selected);

    await activate('Экран помпы');
    await activate('+ Текст');
    await tabTo(button('Применить к LCD'));
    await key('Return', 'display');
    await wait(
      async () =>
        (await rpc('get_lcd_theme', { dev }))?.layers.some(
          (layer) => layer.kind === 'text' && layer.text === 'Новый текст',
        ),
      'Theme IPC readback',
    );
    observations.push('display:add/apply');
    if (window.__nativeDialogs) {
      const imageInput = document.querySelector('.file input[type=file]');
      const initialLayerCount = document.querySelectorAll('.layer').length;
      const beforeImageImport = JSON.stringify(await rpc('get_lcd_theme', { dev }));
      await tabTo(imageInput);
      await key('Tab', 'fixture_file_cancel');
      assert(lastTabTarget === imageInput, 'Image chooser cancel did not restore input focus');
      assert(
        document.querySelectorAll('.layer').length === initialLayerCount,
        'Cancelled chooser changed draft',
      );
      assert(
        JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeImageImport,
        'Cancelled chooser changed applied theme',
      );
      observations.push('image-dialog:cancel/focus/unchanged');
      for (let imported = 1; imported <= 2; imported++) {
        await tabTo(imageInput);
        await key('Tab', 'fixture_file_select');
        await wait(
          () =>
            !imageInput.disabled &&
            document.querySelectorAll('.layer').length === initialLayerCount + imported,
          'Native image import completed',
        );
        assert(
          lastTabTarget === imageInput,
          'Successful import did not restore picker focus before Tab',
        );
        assert(imageInput.value === '', 'Image input must permit same-file reselection');
        assert(
          JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeImageImport,
          'Import must only change draft',
        );
      }
      await activate('Применить к LCD');
      await wait(
        async () =>
          (await rpc('get_lcd_theme', { dev }))?.layers.filter((layer) => layer.kind === 'image')
            .length === 2,
        'Native imported images applied',
      );
      const importedTheme = await rpc('get_lcd_theme', { dev });
      const imageLayers = importedTheme.layers.filter((layer) => layer.kind === 'image');
      importedImageAssets = imageLayers.map((layer) => layer.asset);
      assert(
        importedImageAssets.every((asset) => /^[0-9a-f]{32}$/.test(asset)) &&
          importedImageAssets[0] !== importedImageAssets[1],
        'Imports must have separate managed keys',
      );
      const imagePreview = await rpc('preview_lcd_theme', { dev, theme: importedTheme });
      assert(
        imagePreview.length > 100 && imagePreview[0] === 255 && imagePreview[1] === 216,
        'Imported image theme JPEG preview',
      );
      observations.push('image-dialog:select/reselect/focus/draft/apply');
      await activate('Библиотека');
      const fontDetails = [...document.querySelectorAll('summary')].find(
        (el) => el.textContent.trim() === 'Пользовательские шрифты',
      );
      await tabTo(fontDetails);
      await key('Return');
      const fontInput = field('Загрузить шрифт');
      const beforeFontImport = JSON.stringify(await rpc('get_lcd_theme', { dev }));
      await tabTo(fontInput);
      await key('Tab', 'fixture_file_font_cancel');
      assert(lastTabTarget === fontInput, 'Font chooser cancel did not restore focus');
      assert(
        JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeFontImport,
        'Cancelled font chooser changed theme',
      );
      for (const [kind, errorText] of [
        ['invalid', 'expected a single TrueType/OpenType font'],
        ['large', 'Шрифт должен быть размером от 1 байта до 16 MiB'],
      ]) {
        const beforeChoices = [...document.querySelectorAll('select option[value^="asset:"]')].map(
          (el) => el.value,
        );
        await tabTo(fontInput);
        await key('Tab', `fixture_file_font_${kind}`);
        assert(lastTabTarget === fontInput, `${kind} font error did not retain picker focus`);
        assert(fontInput.value === '' && !fontInput.disabled, 'Rejected font must allow retry');
        assert(
          [...fontDetails.parentElement.querySelectorAll('[role=alert]')].some((el) =>
            el.textContent.includes(errorText),
          ),
          'Expected font error visible',
        );
        assert(
          JSON.stringify(
            [...document.querySelectorAll('select option[value^="asset:"]')].map((el) => el.value),
          ) === JSON.stringify(beforeChoices),
          'Invalid font created a choice',
        );
        assert(
          JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeFontImport,
          'Invalid font changed applied theme',
        );
        inspectReflow(`font-error-${kind}`);
      }
      observations.push('font-dialog:invalid/oversize/focus/unchanged');
      let importedFont;
      for (let imported = 1; imported <= 2; imported++) {
        const beforeOptions = [...document.querySelectorAll('select option[value^="asset:"]')].map(
          (el) => el.value,
        );
        await tabTo(fontInput);
        await key('Tab', 'fixture_file_font_select');
        await wait(
          () =>
            !fontInput.disabled &&
            [...document.querySelectorAll('select option[value^="asset:"]')].some(
              (el) => !beforeOptions.includes(el.value),
            ),
          'New font choice after native upload',
        );
        assert(lastTabTarget === fontInput, 'Font upload did not restore picker focus before Tab');
        assert(fontInput.value === '', 'Font input must permit same-file reselection');
        assert(
          !fontDetails.parentElement.querySelector('[role=alert]'),
          'Valid import clears prior error',
        );
        inspectReflow('font-imported');
        assert(
          JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeFontImport,
          'Font upload must not implicitly apply',
        );
        importedFont = [...document.querySelectorAll('select option[value^="asset:"]')].find(
          (el) => !beforeOptions.includes(el.value),
        );
      }
      await activate('Композиция');
      const fontSelect = importedFont.parentElement;
      await tabTo(fontSelect);
      await key('End');
      await key('Tab');
      assert(
        fontSelect.value === importedFont.value,
        'Keyboard must select the newly imported font',
      );
      const fontKey = fontSelect.value.slice(6);
      assert(/^[0-9a-f]{32}$/.test(fontKey), 'Selected managed font');
      await activate('Применить к LCD');
      await wait(
        async () =>
          (await rpc('get_lcd_theme', { dev }))?.layers.some(
            (layer) => layer.style?.font_asset === fontKey,
          ),
        'Native font selection applied',
      );
      const fontPreview = await rpc('preview_lcd_theme', {
        dev,
        theme: await rpc('get_lcd_theme', { dev }),
      });
      assert(
        fontPreview.length > 100 && fontPreview[0] === 255 && fontPreview[1] === 216,
        'Imported font renders JPEG',
      );
      observations.push('font-dialog:cancel/select/reselect/focus/draft/apply');
    }
    await activate('Экран помпы');
    const resetSummary = [...document.querySelectorAll('summary')].find(
      (el) => el.textContent.trim() === 'Сброс экрана LCD',
    );
    await tabTo(resetSummary);
    await key('Return');
    await wait(
      () => button('Сбросить экран…') && !button('Сбросить экран…').disabled,
      'Reset readback ready',
    );
    const beforeReset = await rpc('get_screen_reset', { dev });
    const beforeProfiles = await rpc('list_profiles');
    const beforeImages = await rpc('list_named_theme_assets');
    const draftName = field('Название').value;
    await activate('Сбросить экран…');
    await activate('Отмена сброса');
    assert(
      JSON.stringify((await rpc('get_screen_reset', { dev })).receipt) ===
        JSON.stringify(beforeReset.receipt),
      'Cancel must not reset',
    );
    await key('Tab', 'fixture_reset_fault');
    await activate('Сбросить экран…');
    await activate('Подтвердить сброс экрана');
    await wait(
      () => button('Повторно применить к LCD') && !button('Повторно применить к LCD').disabled,
      'Failed LCD reset offers explicit reapply',
    );
    const failedReset = await rpc('get_screen_reset', { dev });
    assert(
      failedReset.receipt && failedReset.reapply_available && failedReset.lcd.last_error,
      'Durable reset distinguished from LCD failure',
    );
    assert(
      failedReset.receipt.profile_revision === beforeProfiles.revision + 1,
      'All Screen slots reset once',
    );
    assert(field('Название').value === draftName, 'Reset must preserve editor draft');
    const afterProfiles = await rpc('list_profiles');
    assert(
      afterProfiles.profiles.filter((p) => p.content.kind === 'screen').length === 8,
      'Eight Screen defaults',
    );
    assert(
      JSON.stringify(await rpc('list_named_theme_assets')) === JSON.stringify(beforeImages),
      'Reset retains image files',
    );
    await tabTo(button('Проверить результат сброса'));
    await key('Tab', 'reset');
    await key('Tab', 'fixture_reset_recover');
    await activate('Повторно применить к LCD');
    await wait(async () => {
      const current = await rpc('get_screen_reset', { dev });
      return (
        !current.reapply_available && !current.lcd.last_error && current.lcd.source === 'Theme'
      );
    }, 'Explicit reset reapply restores theme');
    await wait(
      () => document.activeElement === button('Проверить результат сброса'),
      'Reset reapply focus returns to readback',
    );
    assert(
      (await rpc('list_profiles')).revision === afterProfiles.revision,
      'Reapply does not reset slots again',
    );
    const resetState = await rpc('get_lcd_state', { dev });
    assert(
      resetState.applied.brightness_percent === 50 && resetState.applied.rotation_degrees === 0,
      'Applied reset configuration',
    );
    observations.push(
      'screen-reset:keyboard/cancel/commit/lcd-failure/readback/reapply/focus/files/draft',
    );

    await key('Tab', 'fixture_disconnect');
    await wait(
      () =>
        document.querySelector('#connection-status')?.textContent.includes('Нет связи с daemon'),
      'Offline live region',
    );
    assert(button('Применить к LCD').disabled, 'Offline Apply disabled');
    await key('Tab', 'fixture_reconnect');
    await wait(
      () => document.querySelector('#connection-status')?.textContent.includes('Daemon подключён'),
      'Reconnect live region',
    );
    await wait(() => !button('Применить к LCD').disabled, 'Controls recover after reconnect');
    observations.push('connection:offline/reconnected');
    // Keep WebKit alive until the external AT-SPI listener receives the final event.
    await key('Tab', 'fixture_events_ready');

    await invoke('native_smoke_result', {
      result: {
        ok: true,
        observations,
        reflow,
        imported_image_assets: importedImageAssets,
        native_zoom: window.__nativeZoom,
        native_window_focus: true,
        window_focus_events: windowFocusEvents,
        trusted_key_steps: sequence,
        user_agent: navigator.userAgent,
      },
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      result: {
        ok: false,
        error: String(error),
        stack: error.stack,
        observations,
        reflow,
        sequence,
        active: document.activeElement?.outerHTML,
        body: document.body.innerText,
      },
    });
  }
})();
