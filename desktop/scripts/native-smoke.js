(async () => {
  if (window.__nativeSmokeStarted) return;
  window.__nativeSmokeStarted = true;
  const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const delay = (ms) => new Promise((r) => setTimeout(r, ms));
  const assert = (ok, why) => {
    if (!ok) throw new Error(why);
  };
  try {
    let devices = [];
    for (let i = 0; i < 100; i++) {
      devices = await rpc('list_devices');
      if (devices.length) break;
      await delay(100);
    }
    assert(devices.length === 1, 'one mock device');
    const dev = devices[0].id;
    const png = Array.from(
      atob(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==',
      ),
      (c) => c.charCodeAt(0),
    );
    const gradientSettings = () => ({
      colors: [
        [52, 230, 209],
        [26, 115, 105],
      ],
      flip: false,
    });
    // Exercise the production native import normalization command.
    const asset = await invoke('import_image', { bytes: png });
    const theme = {
      version: 1,
      name: 'Native smoke',
      background: [0, 0, 0],
      background_fill: {
        mode: 'horizontal',
        horizontal: gradientSettings(),
        vertical: gradientSettings(),
        radial: gradientSettings(),
      },
      layers: [
        {
          kind: 'image',
          asset,
          x: 0,
          y: 0,
          width: 480,
          height: 480,
          fit: 'contain_native',
          opacity: 255,
          watermark_position: { horizontal: 75, vertical: 50 },
          shadow: { x: 2, y: 2, blur: 2, color: [0, 0, 0], opacity: 60 },
        },
        {
          kind: 'clock',
          format: 'date',
          twenty_four_hour: false,
          show_seconds: false,
          style: { x: 240, y: 120, size: 24, color: [0, 0, 255], align: 'center' },
        },
        {
          kind: 'sensor',
          parts: {
            label: { x: 240, y: 150, size: 22, color: [255, 0, 0], align: 'center' },
            unit: { x: 240, y: 270, size: 22, color: [0, 255, 0], align: 'center' },
          },
          sensor_id: 'fixture:cpu',
          label: 'CPU',
          unit: 'celsius',
          temperature_unit: 'celsius',
          decimals: 1,
          style: {
            line_box: true,
            max_width: 300,
            line_height: 58,
            vertical_align: 'center',
            x: 240,
            y: 200,
            size: 40,
            color: [255, 0, 0],
            align: 'center',
            font: 'mono',
          },
        },
      ],
    };
    // Production native bridge and renderer: unsupported CJK must resolve to
    // distinct fallback glyphs; canonical combining forms must shape identically.
    const textPreview = async (text, font = 'sans') => {
      const sample = {
        version: 1,
        name: 'Native typography',
        background: [0, 0, 0],
        layers: [
          {
            kind: 'text',
            text,
            style: {
              line_box: true,
              x: 40,
              y: 80,
              size: 40,
              color: [255, 255, 255],
              align: 'left',
              font,
            },
          },
        ],
      };
      return JSON.stringify(await rpc('preview_lcd_theme', { theme: sample }));
    };
    assert(
      (await textPreview('溫')) !== (await textPreview('度')),
      'native CJK fallback resolves distinct glyphs',
    );
    assert(
      (await textPreview('é', 'abril_fatface')) === (await textPreview('e\u0301', 'abril_fatface')),
      'native canonical text preserves primary font',
    );
    const before = await rpc('get_lcd_state', { dev });
    const preview = await rpc('preview_lcd_theme', { dev, theme });
    assert(preview[0] === 255 && preview[1] === 216, 'native preview JPEG');
    assert(
      (await rpc('get_lcd_state', { dev })).source === before.source,
      'preview must not apply',
    );
    theme.layers.push({
      kind: 'text',
      text: '繁體中文\tПривет ab\u00adcd',
      style: {
        line_box: true,
        max_width: 220,
        line_height: 28,
        x: 240,
        y: 390,
        size: 24,
        color: [255, 255, 255],
        align: 'center',
        font: 'noto_sans_tc_regular',
      },
    });
    for (let i = 0; i < 3; i++)
      theme.layers.push({
        kind: 'text',
        text: 'LCD 42',
        style: { x: 80 + i * 150, y: 350, size: 20, color: [255, 255, 255], align: 'center' },
      });
    theme.layers.push({
      kind: 'text',
      text: '≒ ◈ \u{30ede}',
      style: {
        x: 240,
        y: 440,
        size: 24,
        color: [255, 255, 255],
        align: 'center',
        font: 'noto_sans_tc_regular',
      },
    });
    // Use actual DOM controls for loading and subsequent Apply.
    await rpc('set_lcd_theme', { dev, theme });
    const button = (name) =>
      Array.from(document.querySelectorAll('button')).find((b) => b.textContent.trim() === name);
    for (let i = 0; i < 100 && !button('Экран помпы'); i++) await delay(100);
    assert(document.querySelector('.devices'), 'native device overview is initial surface');
    button('Экран помпы').click();
    await delay(0);
    assert(button('Композиция') && button('Библиотека'), 'native studio navigation');
    for (let i = 0; i < 100 && !button('Загрузить активную тему'); i++) await delay(100);
    button('Загрузить активную тему').click();
    for (let i = 0; i < 100; i++) {
      if (Array.from(document.querySelectorAll('input')).some((el) => el.value === 'Native smoke'))
        break;
      await delay(100);
    }
    assert(
      Array.from(document.querySelectorAll('input')).some((el) => el.value === 'Native smoke'),
      'native DOM loaded daemon theme',
    );
    const nameInput = Array.from(document.querySelectorAll('input')).find(
      (el) => el.value === 'Native smoke',
    );
    nameInput.value = 'Native DOM applied';
    nameInput.dispatchEvent(new Event('input', { bubbles: true }));
    const backgroundType = Array.from(document.querySelectorAll('label'))
      .find((e) => e.textContent.startsWith('Тип фона'))
      .querySelector('select');
    assert(backgroundType.value === 'horizontal', 'native gradient loaded');
    backgroundType.value = 'radial';
    backgroundType.dispatchEvent(new Event('change', { bubbles: true }));
    await delay(0);
    const flipGradient = Array.from(document.querySelectorAll('label'))
      .find((e) => e.textContent.includes('Обратить градиент'))
      .querySelector('input');
    flipGradient.click();
    const lineBoxToggle = Array.from(document.querySelectorAll('label'))
      .find((e) => e.textContent.includes('Высота блока по строкам (CSS)'))
      .querySelector('input');
    lineBoxToggle.click();
    const tcFontSelect = Array.from(document.querySelectorAll('label'))
      .find((e) => e.textContent.startsWith('Шрифт'))
      .querySelector('select');
    tcFontSelect.value = 'noto_sans_tc_bold';
    tcFontSelect.dispatchEvent(new Event('change', { bubbles: true }));
    button('Библиотека').click();
    await delay(0);
    // Real DOM File input -> chunked native bridge -> production daemon font store.
    const fontDetails = Array.from(document.querySelectorAll('summary')).find(
      (e) => e.textContent === 'Пользовательские шрифты',
    );
    fontDetails.click();
    const licenseDetails = Array.from(document.querySelectorAll('summary')).find(
      (e) => e.textContent === 'Лицензии встроенных шрифтов',
    );
    licenseDetails.click();
    assert(
      ['TypeTogether', 'Caprasimo', 'Cubic 11', 'SIL OPEN FONT LICENSE'].every((value) =>
        licenseDetails.parentElement.textContent.includes(value),
      ),
      'bundled license notices readable',
    );
    licenseDetails.click();
    const fontInput = Array.from(document.querySelectorAll('label'))
      .find((e) => e.textContent.startsWith('Загрузить шрифт'))
      .querySelector('input');
    const fontCleanup = Array.from(document.querySelectorAll('summary')).find(
      (e) => e.textContent === 'Очистка неиспользуемых шрифтов',
    );
    fontCleanup.click();
    button('Проверить неиспользуемые шрифты').click();
    for (let i = 0; i < 100 && !button('Удалить неиспользуемые шрифты'); i++) await delay(100);
    assert(
      fontCleanup.parentElement.textContent.includes('Можно удалить: 1'),
      'native font orphan scan',
    );
    assert(
      fontCleanup.parentElement.textContent.includes('В профилях/настройках: 2'),
      'profile and offline-device fonts protected',
    );
    button('Удалить неиспользуемые шрифты').click();
    for (let i = 0; i < 100 && !fontCleanup.parentElement.textContent.includes('Удалено: 1'); i++)
      await delay(100);
    assert(
      fontCleanup.parentElement.textContent.includes('Удалено: 1'),
      'native font orphan removed',
    );
    assert(
      fontCleanup.parentElement.textContent.includes('В профилях/настройках: 2'),
      'referenced fonts survive native delete',
    );
    const beforeFontImport = JSON.stringify(await rpc('get_lcd_theme', { dev }));
    const badTransfer = new DataTransfer();
    badTransfer.items.add(
      new File([new Uint8Array([1, 2, 3])], 'Broken.ttf', { type: 'font/ttf' }),
    );
    fontInput.files = badTransfer.files;
    fontInput.dispatchEvent(new Event('change', { bubbles: true }));
    for (
      let i = 0;
      i < 100 && (!fontDetails.parentElement.querySelector('[role=alert]') || fontInput.disabled);
      i++
    )
      await delay(100);
    assert(
      fontDetails.parentElement.querySelector('[role=alert]'),
      'native invalid font error visible',
    );
    assert(!fontInput.disabled, 'native invalid font allows explicit retry');
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeFontImport,
      'invalid font preserves active theme',
    );
    assert(nameInput.value === 'Native DOM applied', 'invalid font preserves edited draft');
    const partialTransfer = new DataTransfer();
    partialTransfer.items.add(
      new File([new Uint8Array(window.__nativeFontBytes)], 'Interrupted.ttf', { type: 'font/ttf' }),
    );
    fontInput.files = partialTransfer.files;
    fontInput.dispatchEvent(new Event('change', { bubbles: true }));
    await delay(0);
    for (
      let i = 0;
      i < 100 && (!fontDetails.parentElement.querySelector('[role=alert]') || fontInput.disabled);
      i++
    )
      await delay(100);
    assert(
      fontDetails.parentElement.querySelector('[role=alert]') && !fontInput.disabled,
      'partial upload error permits retry',
    );
    assert(
      !document.body.textContent.includes('Шрифт загружен.'),
      'partial upload never reports success',
    );
    assert(
      !Array.from(document.querySelectorAll('option')).some((o) => o.text === 'Interrupted.ttf'),
      'partial font is not selectable',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeFontImport,
      'partial upload preserves active theme',
    );
    assert(nameInput.value === 'Native DOM applied', 'partial upload preserves edited draft');
    const transfer = new DataTransfer();
    transfer.items.add(
      new File([new Uint8Array(window.__nativeFontBytes)], 'Native Sans.ttf', { type: 'font/ttf' }),
    );
    fontInput.files = transfer.files;
    fontInput.dispatchEvent(new Event('change', { bubbles: true }));
    for (let i = 0; i < 150 && !document.body.textContent.includes('Шрифт загружен.'); i++)
      await delay(100);
    assert(document.body.textContent.includes('Шрифт загружен.'), 'native font file imported');
    const textFont = Array.from(document.querySelectorAll('.layer')[3].querySelectorAll('label'))
      .find((e) => e.textContent.startsWith('Шрифт'))
      .querySelector('select');
    assert(textFont.value === 'noto_sans_tc_regular', 'upload does not select font or apply');
    const fontOption = Array.from(textFont.options).find((o) => o.text === 'Native Sans.ttf');
    assert(fontOption, 'uploaded font is selectable');
    const nativeFontKey = fontOption.value.slice(6);
    textFont.value = fontOption.value;
    textFont.dispatchEvent(new Event('change', { bubbles: true }));
    await delay(0);
    for (const legend of ['Подпись датчика', 'Единица датчика']) {
      const fieldset = Array.from(document.querySelectorAll('fieldset')).find(
        (e) => e.querySelector('legend')?.textContent === legend,
      );
      const select = Array.from(fieldset.querySelectorAll('label'))
        .find((e) => e.textContent.startsWith('Шрифт'))
        .querySelector('select');
      select.value = fontOption.value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }
    await delay(0);
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === beforeFontImport,
      'import and sensor part selection remain draft',
    );
    for (const [i, font] of ['abril_fatface', 'caprasimo', 'cubic11'].entries()) {
      const select = Array.from(
        document.querySelectorAll('.layer')[4 + i].querySelectorAll('label'),
      )
        .find((e) => e.textContent.startsWith('Шрифт'))
        .querySelector('select');
      select.value = font;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }
    await delay(0);
    button('Применить к LCD').click();
    await delay(1200);
    assert((await rpc('get_lcd_state', { dev })).source === 'Theme', 'theme active');
    const a = await rpc('preview_lcd_theme', { dev, theme });
    await delay(450);
    const b = await rpc('preview_lcd_theme', { dev, theme });
    assert(JSON.stringify(a) !== JSON.stringify(b), 'sensor changes rendered pixels');
    const stored = await rpc('get_lcd_theme', { dev });
    assert(
      ['abril_fatface', 'caprasimo', 'cubic11'].every(
        (font, i) => stored.layers[4 + i].style.font === font,
      ),
      'native display fonts applied',
    );
    assert(
      stored.layers[7].text === '≒ ◈ \u{30ede}' &&
        stored.layers[7].style.font === 'noto_sans_tc_regular' &&
        !stored.layers[7].style.font_asset,
      'native Noto coverage layer retained',
    );
    assert(stored.layers[3].style.font_asset === nativeFontKey, 'native imported font applied');
    assert(
      stored.layers[3].style.line_box &&
        stored.layers[3].text === '繁體中文\tПривет ab\u00adcd' &&
        stored.layers[3].style.max_width === 220,
      'native mixed text layout intent applied',
    );
    assert(
      stored.layers[2].parts.label.font_asset === nativeFontKey &&
        stored.layers[2].parts.unit.font_asset === nativeFontKey,
      'native sensor part fonts applied',
    );
    assert(
      stored.layers[1].style.font === 'noto_sans_tc_bold' &&
        stored.layers[3].style.font === 'noto_sans_tc_regular',
      'native TC fonts applied',
    );
    assert(
      stored.layers[1].style.line_box && stored.layers[2].style.line_box,
      'native line box edit and load preserved',
    );
    assert(
      stored.background_fill.mode === 'radial' &&
        stored.background_fill.radial.flip &&
        !stored.background_fill.horizontal.flip,
      'native gradient edit and independent modes',
    );
    assert(
      stored.name === 'Native DOM applied' && stored.layers[0].asset === asset,
      'theme persisted intent',
    );
    const until = async (check, why, attempts = 100) => {
      for (let i = 0; i < attempts; i++) {
        if (await check()) return;
        await delay(100);
      }
      throw new Error('timeout: ' + why);
    };
    const revealPanel = async (el) => {
      const panel = el?.closest('[data-studio-panel]');
      if (panel?.hidden) {
        button(panel.dataset.studioPanel === 'library' ? 'Библиотека' : 'Композиция')?.click();
        await delay(0);
      }
    };
    const clickButton = async (name) => {
      await until(() => button(name) && !button(name).disabled, name);
      await revealPanel(button(name));
      button(name).click();
      await delay(0);
    };
    const field = (name) =>
      Array.from(document.querySelectorAll('label'))
        .find((l) => l.textContent.trim().startsWith(name))
        ?.querySelector('input,select');
    const setField = async (name, value) => {
      await until(() => field(name) && !field(name).disabled, name);
      const el = field(name);
      await revealPanel(el);
      if (el.type === 'checkbox') el.checked = value;
      else el.value = String(value);
      el.dispatchEvent(new Event(el.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }));
      if (el.type === 'checkbox') el.dispatchEvent(new Event('change', { bubbles: true }));
      await delay(0);
    };
    const imageCleanup = Array.from(document.querySelectorAll('summary')).find(
      (e) => e.textContent === 'Очистка неиспользуемых изображений',
    );
    imageCleanup.click();
    await clickButton('Проверить неиспользуемые изображения');
    await until(
      () => imageCleanup.parentElement.textContent.includes('Можно удалить: 1'),
      'image orphan scan',
    );
    assert(
      imageCleanup.parentElement.textContent.includes('В профилях/настройках: 3'),
      'profile and offline-device image protection',
    );
    const cleanupFault = async (mode) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: { action: 'rename', group: 'screen', slot: 6, name: `Cleanup request:${mode}` },
      });
      await until(
        async () =>
          (await rpc('list_profiles')).profiles.some(
            (p) =>
              p.slot === 6 && p.content.kind === 'screen' && p.name === `Cleanup ready:${mode}`,
          ),
        'cleanup fixture acknowledgement',
      );
    };
    // The dry-run was valid, but a reference root breaks before Delete.
    await cleanupFault('corrupt');
    await clickButton('Удалить неиспользуемые изображения');
    await until(
      () => imageCleanup.parentElement.querySelector('[role=alert]'),
      'corrupt settings refuses cleanup',
    );
    assert(
      !button('Удалить неиспользуемые изображения'),
      'failed cleanup invalidates stale deletion preview',
    );
    assert(
      !imageCleanup.parentElement.querySelector('[role=status]'),
      'cleanup failure never claims successful deletion',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'corrupt root does not disturb active theme',
    );
    await cleanupFault('restore');
    await clickButton('Проверить неиспользуемые изображения');
    await until(
      () => imageCleanup.parentElement.textContent.includes('Можно удалить: 1'),
      'orphan still present after failed delete',
    );
    assert(
      !imageCleanup.parentElement.querySelector('[role=alert]'),
      'restored settings clear cleanup error',
    );
    const armProfileCorruption = async () => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: { action: 'rename', group: 'screen', slot: 6, name: 'Native cleanup profile fault' },
      });
      return await rpc('list_profiles');
    };
    const beforeCorruptProfiles = await armProfileCorruption();
    await clickButton('Удалить неиспользуемые изображения');
    await until(
      () => imageCleanup.parentElement.querySelector('[role=alert]'),
      'corrupt profiles refuse image delete',
    );
    assert(
      !button('Удалить неиспользуемые изображения'),
      'corrupt profiles invalidate deletion preview',
    );
    assert(
      !imageCleanup.parentElement.querySelector('[role=status]'),
      'corrupt profiles never claim deletion',
    );
    assert(
      JSON.stringify(await rpc('list_profiles')) === JSON.stringify(beforeCorruptProfiles),
      'profile bytes restored without mutation',
    );
    await clickButton('Проверить неиспользуемые изображения');
    await until(
      () => imageCleanup.parentElement.textContent.includes('Можно удалить: 1'),
      'fresh scan after profile recovery retains orphan',
    );
    await armProfileCorruption();
    await clickButton('Проверить неиспользуемые шрифты');
    await until(
      () => fontCleanup.parentElement.querySelector('[role=alert]'),
      'corrupt profiles refuse font scan',
    );
    assert(
      !fontCleanup.parentElement.querySelector('[role=status]'),
      'font failure clears stale report',
    );
    await clickButton('Проверить неиспользуемые шрифты');
    await until(
      () => fontCleanup.parentElement.textContent.includes('Можно удалить: 0'),
      'font scan recovers after profile restore',
    );
    assert(!fontCleanup.parentElement.querySelector('[role=alert]'), 'font recovery clears error');
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'profile corruption leaves active theme intact',
    );
    await clickButton('Удалить неиспользуемые изображения');
    await until(
      () => imageCleanup.parentElement.textContent.includes('Удалено: 1'),
      'image orphan deleted',
    );
    assert(
      imageCleanup.parentElement.textContent.includes('В профилях/настройках: 3'),
      'referenced images retained during delete',
    );
    assert(
      !imageCleanup.parentElement.querySelector('[role=alert]'),
      'image cleanup has no failures',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'cleanup leaves active theme intact',
    );
    const partialFault = async (kind, phase) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: {
          action: 'rename',
          group: 'screen',
          slot: 6,
          name: `Partial cleanup request:${kind}:${phase}`,
        },
      });
      await until(
        async () =>
          (await rpc('list_profiles')).profiles.some(
            (p) =>
              p.slot === 6 &&
              p.content.kind === 'screen' &&
              p.name === `Partial cleanup ready:${kind}:${phase}`,
          ),
        'partial cleanup fixture acknowledgement',
      );
    };
    for (const [kind, label, details] of [
      ['image', 'изображения', imageCleanup],
      ['font', 'шрифты', fontCleanup],
    ]) {
      await partialFault(kind, 'seed');
      await clickButton(`Проверить неиспользуемые ${label}`);
      await until(
        () => details.parentElement.textContent.includes('Можно удалить: 2'),
        `${kind} two orphan candidates`,
      );
      await clickButton(`Удалить неиспользуемые ${label}`);
      await until(
        () => details.parentElement.querySelector('[role=alert]'),
        `${kind} partial error visible`,
      );
      assert(
        details.parentElement.textContent.includes('Удалено: 1'),
        `${kind} reports successful subset`,
      );
      assert(
        details.parentElement.querySelector('[role=alert]').textContent.includes('f'.repeat(32)),
        `${kind} names failed key`,
      );
      assert(!button(`Удалить неиспользуемые ${label}`), `${kind} retry requires fresh scan`);
      await partialFault(kind, 'restore');
      await clickButton(`Проверить неиспользуемые ${label}`);
      await until(
        () => details.parentElement.textContent.includes('Можно удалить: 1'),
        `${kind} only failed orphan remains`,
      );
      assert(
        !details.parentElement.querySelector('[role=alert]'),
        `${kind} fresh scan clears old failure`,
      );
      await clickButton(`Удалить неиспользуемые ${label}`);
      await until(
        () => details.parentElement.textContent.includes('Удалено: 1'),
        `${kind} retry succeeds`,
      );
      assert(!details.parentElement.querySelector('[role=alert]'), `${kind} retry has no error`);
      assert(
        JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
        `${kind} partial cleanup preserves active theme`,
      );
    }
    const profileSummary = Array.from(document.querySelectorAll('summary')).find(
      (s) => s.textContent.trim() === 'Сохранённые профили',
    );
    assert(profileSummary, 'screen profile editor');
    profileSummary.click();
    await clickButton('Обновить список профилей');
    const retainedProfile = (await rpc('list_profiles')).profiles.find(
      (p) => p.slot === 6 && p.content.kind === 'screen',
    );
    await setField('Слот профиля', 6);
    await clickButton('Загрузить профиль в редактор');
    await until(
      () => field('Название').value === retainedProfile.content.value.name,
      'retained assets loaded as draft without preview',
    );
    await clickButton('Удалить профиль');
    await until(
      async () =>
        !(await rpc('list_profiles')).profiles.some(
          (p) => p.slot === 6 && p.content.kind === 'screen',
        ),
      'profile removed while draft survives',
    );
    for (const [kind, details] of [
      ['изображения', imageCleanup],
      ['шрифты', fontCleanup],
    ]) {
      await clickButton(`Проверить неиспользуемые ${kind}`);
      await until(
        () => details.parentElement.textContent.includes('Можно удалить: 0'),
        `${kind} draft protected`,
      );
      assert(
        details.parentElement.textContent.includes('В черновике: 1'),
        `${kind} retained by draft alone`,
      );
    }
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'loading/deleting profile never applies draft',
    );
    await clickButton('Обновить preview');
    await until(
      () => document.querySelector('.lcd img'),
      'draft assets render after profile deletion',
    );
    await clickButton('Загрузить активную тему');
    await until(
      () => field('Название').value === stored.name,
      'restore active draft after preview',
    );
    for (const [kind, details] of [
      ['изображения', imageCleanup],
      ['шрифты', fontCleanup],
    ]) {
      await clickButton(`Проверить неиспользуемые ${kind}`);
      await until(
        () => details.parentElement.textContent.includes('Можно удалить: 0'),
        `${kind} session protected`,
      );
      assert(
        details.parentElement.textContent.includes('В этом запуске: 1'),
        `${kind} retained by session alone`,
      );
      assert(
        details.parentElement.textContent.includes('В черновике: 0'),
        `${kind} no longer in draft`,
      );
    }
    const restoreLibrary = await rpc('list_profiles');
    await rpc('edit_profile', {
      expected_revision: restoreLibrary.revision,
      edit: { action: 'save', profile: retainedProfile },
    });
    await clickButton('Обновить список профилей');
    await setField('Слот профиля', 0);
    await setField('Имя профиля', 'Native saved screen');
    await clickButton('Сохранить черновик в слот');
    const screenSlot = async () =>
      (await rpc('list_profiles')).profiles.find(
        (p) => p.slot === 0 && p.content.kind === 'screen',
      );
    await until(
      async () => (await screenSlot())?.name === 'Native saved screen',
      'profile save via DOM',
    );
    assert(
      JSON.stringify((await screenSlot()).content.value) === JSON.stringify(stored),
      'saved profile contains full theme',
    );
    // A second IPC client advances the library while the GUI holds its previous revision.
    const previousLibrary = await rpc('list_profiles');
    const externalLibrary = await rpc('edit_profile', {
      expected_revision: previousLibrary.revision,
      edit: { action: 'rename', group: 'screen', slot: 0, name: 'External client rename' },
    });
    await setField('Название', 'Unsaved during conflict');
    await setField('Имя профиля', 'Stale GUI rename');
    await clickButton('Переименовать профиль');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
          el.textContent.includes('profile library changed'),
        ),
      'native revision conflict error',
    );
    assert(
      JSON.stringify(await rpc('list_profiles')) === JSON.stringify(externalLibrary),
      'stale GUI edit cannot overwrite external library',
    );
    assert(
      field('Название').value === 'Unsaved during conflict' &&
        field('Имя профиля').value === 'Stale GUI rename',
      'conflict preserves both drafts',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'conflict cannot mutate active theme',
    );
    await clickButton('Обновить список профилей');
    await until(
      () => field('Слот профиля')?.textContent.includes('External client rename'),
      'refresh sees external edit',
    );
    assert(
      !Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
        el.textContent.includes('profile library changed'),
      ),
      'refresh clears conflict',
    );
    assert(
      field('Название').value === 'Unsaved during conflict' &&
        field('Имя профиля').value === 'Stale GUI rename',
      'refresh preserves both drafts',
    );
    await setField('Имя профиля', 'Native renamed screen');
    await clickButton('Переименовать профиль');
    await until(
      async () => (await screenSlot())?.name === 'Native renamed screen',
      'profile rename via DOM',
    );
    await setField('Название', 'Unsaved local edit');
    await clickButton('Загрузить профиль в редактор');
    await until(
      () => field('Название')?.value === stored.name,
      'profile load replaces local draft',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'profile load does not apply',
    );
    await clickButton('Сбросить слот к базовым настройкам');
    await until(async () => (await screenSlot())?.name === 'Screen 1', 'profile slot reset');
    assert(
      (await screenSlot()).content.value.layers.some((l) => l.kind === 'clock'),
      'reset restores clock default',
    );
    assert(field('Название').value === stored.name, 'slot reset preserves editor draft');
    await clickButton('Удалить профиль');
    await until(async () => !(await screenSlot()), 'profile delete');
    await setField('Имя профиля', 'Native retained screen');
    await clickButton('Сохранить черновик в слот');
    await until(
      async () => (await screenSlot())?.name === 'Native retained screen',
      'profile retained for restart',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'profile mutations never apply to device',
    );
    // Save commits upstream, but the proxy drops its successful reply before the GUI sees it.
    const beforeLostProfile = await rpc('list_profiles');
    await setField('Название', 'Native submitted profile');
    await setField('Имя профиля', 'Native lost profile reply');
    await clickButton('Заменить профиль черновиком');
    await setField('Название', 'Native unsent profile edits');
    await until(
      () =>
        Array.from(document.querySelectorAll('.profiles [role=alert]')).some((el) =>
          el.textContent.includes('response dropped'),
        ),
      'profile lost reply visible',
    );
    assert(
      !document.querySelector('.profiles')?.textContent.includes('Библиотека сохранена.'),
      'lost profile reply not shown as success',
    );
    const lostProfileLibrary = await rpc('list_profiles');
    const lostProfile = lostProfileLibrary.profiles.find(
      (p) => p.slot === 0 && p.content.kind === 'screen',
    );
    assert(
      lostProfileLibrary.revision === beforeLostProfile.revision + 1,
      'lost profile save advances revision exactly once',
    );
    assert(
      lostProfile.name === 'Native lost profile reply' &&
        lostProfile.content.value.name === 'Native submitted profile',
      'saved submitted snapshot, not subsequent edits',
    );
    assert(
      JSON.stringify(lostProfile.content.value) ===
        JSON.stringify({ ...stored, name: 'Native submitted profile' }),
      'full saved theme preserved',
    );
    assert(
      field('Название').value === 'Native unsent profile edits',
      'lost profile reply preserves editor draft',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'lost profile reply cannot apply theme',
    );
    await clickButton('Обновить список профилей');
    await until(
      () => field('Слот профиля')?.textContent.includes('Native lost profile reply'),
      'profile readback after lost reply',
    );
    assert(
      field('Название').value === 'Native unsent profile edits' &&
        field('Имя профиля').value === 'Native lost profile reply',
      'profile refresh preserves both drafts',
    );
    assert(
      JSON.stringify(await rpc('list_profiles')) === JSON.stringify(lostProfileLibrary),
      'readback does not replay save',
    );
    await clickButton('Загрузить профиль в редактор');
    await until(
      () => field('Название')?.value === 'Native submitted profile',
      'explicit load recovers committed profile',
    );
    await setField('Название', stored.name);
    await setField('Имя профиля', 'Native retained screen');
    await clickButton('Заменить профиль черновиком');
    await until(
      async () => (await screenSlot())?.name === 'Native retained screen',
      'explicit profile restore after lost reply',
    );
    assert(
      JSON.stringify((await screenSlot()).content.value) === JSON.stringify(stored),
      'explicit profile restore retains full theme',
    );
    // Reserve screen slot 8 for proxy faults; the active theme remains untouched.
    const profileFaultRecovery = {};
    for (const action of ['rename', 'reset', 'delete', 'reject']) {
      const beforeSeed = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: beforeSeed.revision,
        edit: {
          action: 'save',
          profile: {
            slot: 7,
            name: 'Native fault slot',
            content: { kind: 'screen', value: stored },
          },
        },
      });
      await clickButton('Обновить список профилей');
      await setField('Слот профиля', 7);
      await setField(
        'Имя профиля',
        action === 'reject' ? 'Native reject before forward' : 'Native fault renamed',
      );
      await setField('Название', `Unsent during ${action}`);
      const before = await rpc('list_profiles');
      await clickButton(
        {
          rename: 'Переименовать профиль',
          reset: 'Сбросить слот к базовым настройкам',
          delete: 'Удалить профиль',
          reject: 'Заменить профиль черновиком',
        }[action],
      );
      // A shared socket failure may also invalidate health polling. Reconnect
      // clears this component's library; either path must require explicit readback.
      await until(
        () =>
          !field('Слот профиля') ||
          Array.from(document.querySelectorAll('.profiles [role=alert]')).some((el) =>
            el.textContent.includes('response dropped'),
          ),
        `${action} requires profile readback`,
      );
      profileFaultRecovery[action] = field('Слот профиля')
        ? 'mutation-error'
        : 'connection-invalidated-library';
      assert(
        !button('Загрузить профиль в редактор') || button('Загрузить профиль в редактор').disabled,
        'stale profile load blocked',
      );
      assert(
        !button('Заменить профиль черновиком') || button('Заменить профиль черновиком').disabled,
        'ambiguous profile retry blocked',
      );
      assert(
        !button('Сохранить черновик в слот') || button('Сохранить черновик в слот').disabled,
        'empty library cannot retry save',
      );
      assert(
        !Array.from(document.querySelectorAll('.profiles [role=status]')).some((el) =>
          el.textContent.includes('Библиотека сохранена.'),
        ),
        'failed reply cannot show save success',
      );
      assert(field('Название').value === `Unsent during ${action}`, 'fault preserves unsent theme');
      assert(
        JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
        'profile fault does not apply LCD',
      );
      const after = await rpc('list_profiles');
      const slot = after.profiles.find((p) => p.slot === 7 && p.content.kind === 'screen');
      assert(
        after.revision === before.revision + (action === 'reject' ? 0 : 1),
        'one commit or no pre-forward commit',
      );
      if (action === 'reject')
        assert(
          JSON.stringify(after) === JSON.stringify(before),
          'pre-forward failure preserves full library',
        );
      if (action === 'rename')
        assert(
          slot.name === 'Native fault renamed' &&
            JSON.stringify(slot.content.value) === JSON.stringify(stored),
          'rename committed without changing content',
        );
      if (action === 'delete') assert(!slot, 'delete committed');
      if (action === 'reset')
        assert(
          slot.name === 'Screen 8' && slot.content.value.layers.some((l) => l.kind === 'clock'),
          'reset committed defaults',
        );
      await clickButton('Обновить список профилей');
      assert(
        field('Название').value === `Unsent during ${action}`,
        'readback never overwrites theme draft',
      );
      assert(
        !document.querySelector('.profiles [role=alert]'),
        'explicit readback clears mutation error',
      );
      if (action === 'delete')
        assert(
          button('Загрузить профиль в редактор').disabled,
          'deleted slot stays empty after readback',
        );
      else {
        await clickButton('Загрузить профиль в редактор');
        await until(
          () => field('Название').value === slot.content.value.name,
          'explicit readback load',
        );
      }
    }
    // Subsequent dependency scenarios use the original saved slot.
    await setField('Слот профиля', 0);
    await clickButton('Загрузить профиль в редактор');
    const dependencyFault = async (kind, mode) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: {
          action: 'rename',
          group: 'screen',
          slot: 0,
          name: `Dependency request:${kind}:${mode}`,
        },
      });
      await until(
        async () => (await screenSlot())?.name === `Dependency ready:${kind}:${mode}`,
        'dependency fixture acknowledgement',
      );
      await clickButton('Обновить список профилей');
      await clickButton('Загрузить профиль в редактор');
      await until(() => field('Название')?.value === stored.name, 'saved dependency profile load');
    };
    for (const kind of ['font', 'image'])
      for (const mode of ['missing', 'corrupt']) {
        await dependencyFault(kind, mode);
        const savedLibrary = await rpc('list_profiles');
        await setField('Название', `Unsaved ${kind} ${mode}`);
        await clickButton('Применить к LCD');
        await until(
          () => document.querySelector('.error')?.textContent.length > 0,
          'saved dependency error visible',
        );
        assert(
          !document.body.textContent.includes('Тема отправлена.'),
          'failed dependency never reports Apply success',
        );
        assert(
          field('Название').value === `Unsaved ${kind} ${mode}`,
          'dependency failure preserves draft',
        );
        assert(
          JSON.stringify(await rpc('list_profiles')) === JSON.stringify(savedLibrary),
          'dependency failure preserves profile and revision',
        );
        assert(
          JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
          'dependency failure preserves active theme',
        );
        assert(
          (await rpc('get_lcd_state', { dev })).source === 'Theme',
          'prepared runtime survives dependency loss',
        );
        await dependencyFault(kind, `restore-${mode}`);
        await clickButton('Применить к LCD');
        await until(
          () => document.body.textContent.includes('Тема отправлена.'),
          'restored dependency can Apply',
        );
        assert(!document.querySelector('.error'), 'dependency recovery clears error');
        assert(
          JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
          'restored dependency uses original theme',
        );
      }
    const restartDependency = async (kind, mode) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: {
          action: 'rename',
          group: 'screen',
          slot: 0,
          name: `Restart dependency request:${kind}:${mode}`,
        },
      });
      await until(
        () => document.querySelector('.connection')?.textContent.includes('Нет связи с daemon'),
        'dependency restart offline',
      );
      await until(
        () => document.querySelector('.connection')?.textContent.includes('Daemon подключён'),
        'dependency restart online',
      );
      await until(
        async () => (await screenSlot())?.name === `Restart dependency ready:${kind}:${mode}`,
        'restart dependency acknowledgement',
      );
    };
    for (const kind of ['font', 'image']) {
      await setField('Название', `Draft across missing ${kind}`);
      await restartDependency(kind, 'missing');
      const failed = await rpc('get_lcd_state', { dev });
      assert(
        failed.source === 'Idle' && failed.last_error?.includes('theme restore:'),
        'missing dependency is failed replay, not Theme',
      );
      assert(
        (await rpc('get_lcd_theme', { dev })) === null,
        'missing dependency has no active theme runtime',
      );
      await until(
        () => document.body.textContent.includes('theme restore:'),
        'restore error visible in native GUI',
      );
      assert(
        field('Название').value === `Draft across missing ${kind}`,
        'failed replay preserves editor draft',
      );
      assert(
        JSON.stringify((await screenSlot()).content.value) === JSON.stringify(stored),
        'failed replay preserves saved profile',
      );
      await restartDependency(kind, 'restore');
      const recovered = await rpc('get_lcd_state', { dev });
      assert(
        recovered.source === 'Theme' && !recovered.last_error,
        'restored dependency replays without Apply',
      );
      assert(
        JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
        'restored dependency uses full saved theme',
      );
      assert(
        field('Название').value === `Draft across missing ${kind}`,
        'recovery does not apply draft',
      );
      await clickButton('Загрузить активную тему');
      await until(() => field('Название')?.value === stored.name, 'load recovered theme');
    }
    const missingTheme = JSON.parse(JSON.stringify(stored));
    missingTheme.layers[0].asset = '00000000000000000000000000000000';
    const intactLibrary = await rpc('list_profiles');
    const rejectsMissingAsset = async (action, why) => {
      try {
        await action();
      } catch (error) {
        assert(
          String(error?.message ?? error).includes('No such file'),
          why + ' returns missing-file error',
        );
        return;
      }
      throw new Error(why + ' unexpectedly succeeded');
    };
    await rejectsMissingAsset(
      () =>
        rpc('edit_profile', {
          expected_revision: intactLibrary.revision,
          edit: {
            action: 'save',
            profile: {
              slot: 1,
              name: 'Missing asset',
              content: { kind: 'screen', value: missingTheme },
            },
          },
        }),
      'missing asset profile save',
    );
    assert(
      JSON.stringify(await rpc('list_profiles')) === JSON.stringify(intactLibrary),
      'failed asset save preserves full library/revision',
    );
    await rejectsMissingAsset(
      () => rpc('set_lcd_theme', { dev, theme: missingTheme }),
      'missing asset theme apply',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'failed asset apply preserves active theme',
    );
    const saveCoolingProfile = async (group, editField, changed, original) => {
      const summary = Array.from(document.querySelectorAll('summary')).find(
        (s) => s.textContent.trim() === 'Сохранённые профили',
      );
      assert(summary, 'cooling profile editor');
      if (!summary.parentElement.open) summary.click();
      await clickButton('Обновить список профилей');
      await setField('Имя профиля', `Native ${group} retained`);
      await clickButton('Сохранить черновик в слот');
      await until(
        async () =>
          (await rpc('list_profiles')).profiles.some(
            (p) =>
              p.slot === 0 && p.content.kind === group && p.name === `Native ${group} retained`,
          ),
        `${group} profile save`,
      );
      const before = (await rpc('get_cooling_state', { dev }))[group].requested;
      const saved = (await rpc('list_profiles')).profiles.find(
        (p) => p.slot === 0 && p.content.kind === group,
      );
      assert(
        JSON.stringify(saved.content.value) === JSON.stringify(before),
        `${group} full profile content`,
      );
      await setField(editField, changed);
      await clickButton('Загрузить профиль в редактор');
      await until(() => field(editField)?.value === String(original), `${group} profile load`);
      assert(
        JSON.stringify((await rpc('get_cooling_state', { dev }))[group].requested) ===
          JSON.stringify(before),
        `${group} profile load does not apply`,
      );
    };
    await clickButton('Датчики');
    await until(
      () =>
        Array.from(document.querySelectorAll('.sensors article')).some(
          (el) =>
            el.querySelector('h2')?.textContent === 'Fixture CPU' &&
            el.querySelector('strong')?.textContent.includes('°C'),
        ),
      'sensor catalog UI',
    );
    const sourceEditor = () =>
      document.querySelector('section[aria-label="Источники системных датчиков"]');
    await clickButton('Восстановить рекомендуемый выбор');
    assert(
      field('Системная видеокарта').value === 'sysfs:2f60:i915',
      'native recommended Intel GPU draft',
    );
    assert(
      field('CPU · Температура').value === 'hwmon:2f63:coretemp:temp1',
      'native recommended CPU draft',
    );
    assert(
      (await rpc('get_sensor_defaults')).config.revision === 0,
      'native recommendation does not save',
    );
    assert(
      field('GPU · Загрузка').value === '',
      'native recommendation never invents Intel aggregate',
    );
    assert(
      field('CPU · Вентилятор').value === '',
      'native recommendation leaves CPU fan unassigned',
    );
    await setField('CPU · Вентилятор', 'hwmon:2f64:nct6775:fan1');
    await setField('Системная видеокарта', 'sysfs:2f62:amdgpu');
    await setField('GPU · Загрузка', 'amdgpu:2f62:utilization');
    assert(
      !Array.from(field('GPU · Загрузка').options).some(
        (o) => o.value === 'amdgpu:2f61:utilization',
      ),
      'native GPU candidates isolated',
    );
    assert((await rpc('get_sensor_defaults')).config.revision === 0, 'native choice remains draft');
    const sourceThemeBefore = await rpc('get_lcd_theme', { dev });
    await clickButton('Применить источники');
    await until(
      () => sourceEditor()?.textContent.includes('Источники сохранены'),
      'native source save',
    );
    let sensorDefaults = (await rpc('get_sensor_defaults')).config;
    assert(
      sensorDefaults.revision === 1 && sensorDefaults.gpu.load === 'amdgpu:2f62:utilization',
      'native source persisted',
    );
    await until(
      () =>
        sourceEditor() &&
        Array.from(sourceEditor().querySelectorAll('dd')).some((e) => e.textContent === '35.0%'),
      'native resolved GPU B value',
    );
    assert(
      sensorDefaults.cpu.temperature === 'hwmon:2f63:coretemp:temp1',
      'native CPU recommended binding saved',
    );
    assert(sensorDefaults.cpu.fan === 'hwmon:2f64:nct6775:fan1', 'native explicit CPU fan saved');
    await until(
      () =>
        Array.from(sourceEditor().querySelectorAll('dd')).some((e) => e.textContent === '1234 RPM'),
      'native CPU fan value',
    );
    assert(
      Array.from(sourceEditor().querySelectorAll('dd')).some((e) => e.textContent === '45.0 °C'),
      'native resolved CPU value',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(sourceThemeBefore),
      'global defaults do not rewrite explicit LCD theme',
    );
    const explicitPolicies = async () => ({
      theme: await rpc('get_lcd_theme', { dev }),
      pump: (await rpc('get_cooling_state', { dev })).pump.requested,
      fan: (await rpc('get_cooling_state', { dev })).fan.requested,
      thermal: (await rpc('get_thermal_state', { dev })).requested,
      dynamic: (await rpc('get_dynamic_rgb', { dev })).requested,
    });
    const policiesBefore = await explicitPolicies();
    await setField('Системная видеокарта', 'sysfs:2f61:amdgpu');
    await setField('GPU · Загрузка', 'amdgpu:2f61:utilization');
    await clickButton('Применить источники');
    await until(
      () =>
        Array.from(sourceEditor().querySelectorAll('[role=alert]')).some((e) =>
          e.textContent.includes('response dropped'),
        ),
      'source lost reply is visible',
    );
    assert(
      !sourceEditor().textContent.includes('Источники сохранены'),
      'lost source reply not shown as success',
    );
    assert(
      field('Системная видеокарта').value === 'sysfs:2f61:amdgpu',
      'lost source reply preserves draft',
    );
    await until(
      () => sourceEditor().textContent.includes('Настройки изменились извне'),
      'source readback revision conflict',
    );
    assert(button('Применить источники').disabled, 'stale source revision cannot be reapplied');
    const afterLostSource = (await rpc('get_sensor_defaults')).config;
    assert(
      afterLostSource.revision === 2 && afterLostSource.gpu.load === 'amdgpu:2f61:utilization',
      'lost source reply was committed',
    );
    assert(
      JSON.stringify(await explicitPolicies()) === JSON.stringify(policiesBefore),
      'source error does not rewrite active policies',
    );
    await clickButton('Загрузить сохранённый выбор');
    assert(
      field('GPU · Загрузка').value === 'amdgpu:2f61:utilization',
      'explicit source readback loads committed selection',
    );
    await setField('Системная видеокарта', 'sysfs:2f62:amdgpu');
    await setField('GPU · Загрузка', 'amdgpu:2f62:utilization');
    await clickButton('Применить источники');
    await until(
      () => sourceEditor().textContent.includes('Источники сохранены'),
      'source explicit recovery save',
    );
    sensorDefaults = (await rpc('get_sensor_defaults')).config;
    assert(
      sensorDefaults.revision === 3 && sensorDefaults.gpu.load === 'amdgpu:2f62:utilization',
      'source recovery persisted once',
    );
    assert(
      sensorDefaults.cpu.temperature === 'hwmon:2f63:coretemp:temp1',
      'source recovery preserves CPU',
    );
    assert(
      JSON.stringify(await explicitPolicies()) === JSON.stringify(policiesBefore),
      'source recovery preserves active policies',
    );
    for (const [gpu, engine, memory, label, value, vram] of [
      [
        'sysfs:2f60:i915',
        'intel:2f60:i915:engine:0:utilization',
        'intel-memory:2f60:i915:vram_used',
        'Fixture Intel i915 render engine',
        '27.0%',
        '1.0 GiB',
      ],
      [
        'sysfs:2f65:xe',
        'intel:2f65:xe:gt0:class0:instance0:utilization',
        'intel-memory:2f65:xe:vram_used',
        'Fixture Intel xe GT0 render engine',
        '47.0%',
        '2.0 GiB',
      ],
    ]) {
      await setField('Системная видеокарта', gpu);
      assert(field('GPU · Загрузка').value === '', 'switch GPU clears previous load');
      assert(
        Array.from(field('GPU · Загрузка').options).some(
          (o) => o.value === engine && o.textContent.includes(label),
        ),
        'Intel engine readable option',
      );
      assert(
        Array.from(field('GPU · Загрузка').options).every((o) => !o.value || o.value === engine),
        'Intel engine options belong to selected GPU',
      );
      await setField('GPU · Загрузка', engine);
      await setField('GPU · Использование VRAM', memory);
      const revision = sensorDefaults.revision;
      await clickButton('Применить источники');
      await until(
        async () => (await rpc('get_sensor_defaults')).config.revision === revision + 1,
        'Intel binding committed',
      );
      sensorDefaults = (await rpc('get_sensor_defaults')).config;
      assert(
        sensorDefaults.gpu.load === engine && sensorDefaults.gpu.memory_used === memory,
        'Intel engine and VRAM saved',
      );
      await until(
        () =>
          Array.from(sourceEditor().querySelectorAll('dd')).some((e) => e.textContent === value),
        'Intel resolved load',
      );
      await until(
        () => Array.from(sourceEditor().querySelectorAll('dd')).some((e) => e.textContent === vram),
        'Intel resolved VRAM',
      );
      assert(
        Array.from(field('GPU · Использование VRAM').options).every(
          (o) => !o.value || o.value === memory,
        ),
        'VRAM options belong to selected GPU',
      );
      if (gpu === 'sysfs:2f60:i915') {
        const library = await rpc('list_profiles');
        await rpc('edit_profile', {
          expected_revision: library.revision,
          edit: { action: 'rename', group: 'screen', slot: 0, name: 'Native reconnect intel' },
        });
        await until(
          () => document.querySelector('.connection')?.textContent.includes('Нет связи с daemon'),
          'i915 restart offline',
        );
        await until(
          () => document.querySelector('.connection')?.textContent.includes('Daemon подключён'),
          'i915 restart online',
        );
        await until(
          () => !button('Загрузить сохранённый выбор').disabled,
          'i915 fresh read after restart',
        );
        assert(
          JSON.stringify((await rpc('get_sensor_defaults')).config) ===
            JSON.stringify(sensorDefaults),
          'i915 engine/VRAM survive restart',
        );
        await clickButton('Загрузить сохранённый выбор');
        assert(
          field('GPU · Загрузка').value === engine &&
            field('GPU · Использование VRAM').value === memory,
          'i915 readback after restart',
        );
      }
      assert(
        JSON.stringify(await explicitPolicies()) === JSON.stringify(policiesBefore),
        'Intel defaults preserve explicit policies',
      );
    }
    await clickButton('Восстановить рекомендуемый выбор');
    assert(
      field('Системная видеокарта').value === 'sysfs:2f60:i915' &&
        field('GPU · Загрузка').value === '',
      'recommended reset leaves Intel engine unassigned',
    );
    assert(
      JSON.stringify((await rpc('get_sensor_defaults')).config) === JSON.stringify(sensorDefaults),
      'recommended reset remains draft',
    );
    await clickButton('Очистить выбор в черновике');
    assert(field('Системная видеокарта').value === '', 'native clear draft');
    assert(
      JSON.stringify((await rpc('get_sensor_defaults')).config) === JSON.stringify(sensorDefaults),
      'clear does not write',
    );
    await clickButton('Охлаждение');
    let nativeCoolingModes = await rpc('get_cooling_modes', { dev });
    if (window.__nativeCoolingDefaults) {
      assert(nativeCoolingModes === null, 'fresh fixture has no seeded mode bank');
      const beforeInit = await rpc('get_cooling_state', { dev });
      const summary = document.querySelector('.device-modes > summary');
      assert(summary, 'native standard mode initializer');
      summary.click();
      await clickButton('Прочитать режимы устройства');
      await until(() => field('Источник стандартных режимов'), 'native empty bank controls');
      await setField('Источник стандартных режимов', 'fixture:cpu');
      await clickButton('Создать стандартные режимы');
      await until(
        () =>
          document
            .querySelector('.device-modes')
            ?.textContent.includes('Найдены сохранённые режимы.'),
        'native initialization readback after lost reply',
      );
      nativeCoolingModes = await rpc('get_cooling_modes', { dev });
      assert(
        nativeCoolingModes.fan.length === 8 &&
          nativeCoolingModes.pump.length === 3 &&
          nativeCoolingModes.pump.every((m) => m.use_default_name),
        'native production default bank and labels',
      );
      const afterInit = await rpc('get_cooling_state', { dev });
      for (const channel of ['fan', 'pump'])
        assert(
          JSON.stringify(beforeInit[channel].requested) ===
            JSON.stringify(afterInit[channel].requested) &&
            beforeInit[channel].mode_id === afterInit[channel].mode_id,
          'native initialization does not Apply',
        );
    }

    if (nativeCoolingModes) {
      assert(
        nativeCoolingModes.fan.length === 8 &&
          nativeCoolingModes.pump.length === 3 &&
          !('source' in nativeCoolingModes),
        'native mode list and metadata omission',
      );
      const beforeModes = await rpc('get_cooling_state', { dev });
      const summary = document.querySelector('.device-modes > summary');
      assert(summary, 'native device mode editor');
      if (!summary.parentElement.open) summary.click();
      await clickButton('Прочитать режимы устройства');
      await until(
        () => field('Сохранённый режим')?.options.length === 4,
        'native three pump modes',
      );
      await setField('Сохранённый режим', 'FixRPM');
      await clickButton('Загрузить режим в черновик');
      await until(
        () =>
          field('Режим')?.value === 'fixed_rpm' &&
          Number(field('Целевые RPM (reference mapping)')?.value) ===
            (window.__nativeCoolingDefaults ? 2200 : 2300),
        'native saved mode draft',
      );
      assert(
        JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
          JSON.stringify(beforeModes.pump.requested),
        'native mode load does not apply',
      );
      const bankBeforeRestore = await rpc('get_cooling_modes', { dev });
      await setField('Целевые RPM (reference mapping)', 2800);
      await clickButton('Стандартные параметры в черновик');
      await until(
        () => Number(field('Целевые RPM (reference mapping)')?.value) === 2200,
        'native factory draft restore',
      );
      await setField('Имя режима', 'Temporary draft name');
      await clickButton('Вернуть стандартное имя');
      assert(
        field('Имя режима').value === 'Фиксированные RPM',
        'native factory name restored locally',
      );
      assert(
        JSON.stringify(await rpc('get_cooling_modes', { dev })) ===
          JSON.stringify(bankBeforeRestore),
        'native draft restore does not Save',
      );
      assert(
        JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
          JSON.stringify(beforeModes.pump.requested),
        'native draft restore does not Apply',
      );
      await setField('Имя режима', 'Native saved pump');
      await setField('Целевые RPM (reference mapping)', 2400);
      await clickButton('Сохранить снимок режима');
      await until(
        () =>
          document
            .querySelector('.device-modes [role="status"]')
            ?.textContent.includes('Native saved pump'),
        'native stored mode acknowledgement',
      );
      nativeCoolingModes = await rpc('get_cooling_modes', { dev });
      const savedMode = nativeCoolingModes.pump.find((mode) => mode.id === 'FixRPM');
      assert(
        savedMode.name === 'Native saved pump' &&
          savedMode.policy.rpm === 2400 &&
          !nativeCoolingModes.persistence.pending &&
          !nativeCoolingModes.persistence.last_error,
        'native saved snapshot durable',
      );
      assert(
        JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
          JSON.stringify(beforeModes.pump.requested),
        'native Save does not apply',
      );
      await clickButton('Применить охлаждение');
      await until(async () => {
        const pump = (await rpc('get_cooling_state', { dev })).pump;
        return (
          pump.requested?.mode === 'fixed_rpm' &&
          pump.requested.rpm === 2400 &&
          pump.mode_id === 'FixRPM' &&
          pump.applied_percent === 60 &&
          !pump.last_error
        );
      }, 'native saved RPM mode explicit Apply');
      const modeLibrary = await rpc('list_profiles', {});
      await rpc('edit_profile', {
        expected_revision: modeLibrary.revision,
        edit: { action: 'rename', group: 'screen', slot: 0, name: 'Cooling mode restart request' },
      });
      await until(
        () => document.querySelector('.connection')?.textContent.includes('Нет связи с daemon'),
        'stored mode restart offline',
      );
      await until(
        () => document.querySelector('.connection')?.textContent.includes('Daemon подключён'),
        'stored mode restart online',
      );
      await until(
        async () => (await screenSlot())?.name === 'Cooling mode restart ready',
        'stored mode restart acknowledgement',
      );
      await until(async () => {
        const pump = (await rpc('get_cooling_state', { dev })).pump;
        return (
          pump.mode_id === 'FixRPM' &&
          pump.requested?.rpm === 2400 &&
          pump.applied_percent === 60 &&
          !pump.last_error
        );
      }, 'stored mode identity and snapshot replay');
      await until(
        () => document.body.textContent.includes('Запрошенный режим устройства: FixRPM'),
        'native stored identity label after restart',
      );
    }
    const beforeCooling = await rpc('get_cooling_state', { dev });
    await setField('Источник температуры', 'fixture:cpu');
    await clickButton('quiet');
    await until(() => field('Режим')?.value === 'curve_rpm', 'native preset bridge');
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
        JSON.stringify(beforeCooling.pump.requested),
      'preset stays a draft',
    );
    await setField('Точка 1: °C', 35);
    await clickButton('Применить охлаждение');
    await until(
      async () =>
        (await rpc('get_cooling_state', { dev })).pump.requested?.points?.[0]?.celsius === 35,
      'pump curve apply',
    );
    await saveCoolingProfile('pump', 'Точка 1: °C', 36, 35);
    const knownPump = (await rpc('get_cooling_state', { dev })).pump.requested;
    const missingPump = { ...knownPump, sensor_id: 'fixture:missing-cpu', fallback_percent: 100 };
    const pumpLibrary = await rpc('list_profiles');
    await rpc('edit_profile', {
      expected_revision: pumpLibrary.revision,
      edit: {
        action: 'save',
        profile: {
          slot: 1,
          name: 'Missing sensor pump',
          content: { kind: 'pump', value: missingPump },
        },
      },
    });
    await clickButton('Обновить список профилей');
    await setField('Слот профиля', 1);
    await clickButton('Загрузить профиль в редактор');
    await until(
      () =>
        field('Источник температуры')?.selectedOptions[0]?.textContent.includes(
          'Недоступен: fixture:missing-cpu',
        ),
      'missing sensor remains explicit in draft',
    );
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
        JSON.stringify(knownPump),
      'missing sensor profile load cannot apply',
    );
    await clickButton('Применить охлаждение');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
          el.textContent.includes('binding must identify a known host'),
        ),
      'unknown sensor Apply rejected visibly',
    );
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
        JSON.stringify(knownPump),
      'unknown sensor Apply preserves working policy',
    );
    await setField('Слот профиля', 0);
    await clickButton('Загрузить профиль в редактор');
    await clickButton('Применить охлаждение');
    await until(async () => {
      const state = (await rpc('get_cooling_state', { dev })).pump;
      return (
        JSON.stringify(state.requested) === JSON.stringify(knownPump) && !state.fallback_active
      );
    }, 'restore valid pump binding');
    await setField('Слот профиля', 1);
    await clickButton('Удалить профиль');
    await until(
      async () =>
        !(await rpc('list_profiles')).profiles.some(
          (p) => p.slot === 1 && p.content.kind === 'pump',
        ),
      'remove missing sensor fixture profile',
    );

    const sensorFault = async (mode) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: { action: 'rename', group: 'screen', slot: 0, name: `Sensor request:${mode}` },
      });
      await until(
        async () => (await screenSlot())?.name === `Sensor ready:${mode}`,
        'sensor fixture acknowledgement',
      );
    };
    await setField('Точка 1: °C', 36);
    await sensorFault('missing');
    await until(async () => {
      const state = (await rpc('get_cooling_state', { dev })).pump;
      return state.fallback_active && state.applied_percent === knownPump.fallback_percent;
    }, 'active sensor loss uses configured fallback');
    await until(
      () =>
        document.body.textContent.includes('Fallback активен') &&
        field('Источник температуры')?.selectedOptions[0]?.textContent.includes(
          'Недоступен: fixture:cpu',
        ),
      'GUI shows fallback and unavailable binding',
    );
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
        JSON.stringify(knownPump),
      'sensor loss preserves active policy',
    );
    assert(field('Точка 1: °C').value === '36', 'sensor loss preserves unsaved curve');
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'sensor loss preserves LCD bindings',
    );
    await sensorFault('restore');
    await until(async () => {
      const state = (await rpc('get_cooling_state', { dev })).pump;
      return (
        !state.fallback_active &&
        state.applied_percent !== null &&
        state.applied_percent !== knownPump.fallback_percent
      );
    }, 'same sensor recovers curve output');
    await until(
      () =>
        field('Источник температуры')?.selectedOptions[0]?.textContent.includes(
          'Fixture CPU · valid',
        ) && document.body.textContent.includes('Fallback не активен'),
      'GUI shows restored original sensor',
    );
    assert(
      field('Источник температуры').value === 'fixture:cpu' && field('Точка 1: °C').value === '36',
      'sensor recovery preserves binding and draft',
    );
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).pump.requested) ===
        JSON.stringify(knownPump),
      'recovery does not apply draft',
    );
    await setField('Точка 1: °C', 35);
    await setField('Канал', 'fan');
    await until(() => field('Режим')?.value === 'fixed', 'fan mode');
    await setField('PWM, %', 55);
    await clickButton('Применить охлаждение');
    await until(
      async () => (await rpc('get_cooling_state', { dev })).fan.requested?.percent === 55,
      'fan fixed apply',
    );
    await until(async () => {
      const state = await rpc('get_cooling_state', { dev });
      return (
        state.fan.applied_percent === 55 &&
        state.pump.applied_percent !== null &&
        !state.pump.fallback_active
      );
    }, 'cooling applied output');
    await saveCoolingProfile('fan', 'PWM, %', 56, 55);
    const cooling = await rpc('get_cooling_state', { dev });
    assert(
      !cooling.pump.last_error && !cooling.fan.last_error,
      'mock cooling applied without error',
    );

    await clickButton('Настройки');
    await clickButton('Загрузить текущие значения');
    const beforeLcd = (await rpc('get_lcd_state', { dev })).requested;
    await setField('Ориентация', 180);
    await clickButton('Применить поворот');
    await until(
      async () => (await rpc('get_lcd_state', { dev })).requested.rotation_degrees === 180,
      'rotation patch',
    );
    assert(
      (await rpc('get_lcd_state', { dev })).requested.brightness_percent ===
        beforeLcd.brightness_percent,
      'rotation preserves brightness',
    );
    await setField('Яркость LCD:', 65);
    await clickButton('Применить яркость');
    await until(
      async () => (await rpc('get_lcd_state', { dev })).requested.brightness_percent === 65,
      'brightness patch',
    );
    const lcd = await rpc('get_lcd_state', { dev });
    assert(
      lcd.source === 'Theme' && lcd.requested.rotation_degrees === 180,
      'settings preserve source and rotation',
    );

    await clickButton('Подсветка');
    await until(
      () => Array.from(field('Эффект')?.options ?? []).some((o) => o.value === 'breathing'),
      'lighting catalog',
    );
    await setField('Эффект', 'breathing');
    await clickButton('Применить подсветку');
    await until(
      async () => (await rpc('get_dynamic_rgb', { dev })).base_mode === 'breathing',
      'manual lighting base',
    );
    await setField('Зона', 'fan');
    const fanCatalog = await rpc('get_lighting_info', { dev, kind: 'fan' });
    const fanMode = fanCatalog.modes.includes('breathing') ? 'breathing' : fanCatalog.modes[0];
    await until(
      () => Array.from(field('Эффект')?.options ?? []).some((o) => o.value === fanMode),
      'fan lighting catalog',
    );
    await setField('Эффект', fanMode);
    await clickButton('Применить подсветку');
    await until(
      async () => (await rpc('get_thermal_state', { dev })).fan.applied?.kind === 'base',
      'fan lighting base',
    );
    await setField('Зона', 'pump');
    await clickButton('Загрузить текущие оповещения');
    await setField('Включить CPU', true);
    await setField('Датчик CPU', 'fixture:cpu');
    await setField('Порог CPU,', 80);
    await clickButton('Применить оповещения');
    await until(
      async () => (await rpc('get_thermal_state', { dev })).requested.cpu?.threshold_c === 80,
      'thermal config',
    );
    const thermal = await rpc('get_thermal_state', { dev });
    const dynamicSummary = Array.from(document.querySelectorAll('summary')).find(
      (s) => s.textContent.trim() === 'Подсветка помпы по датчику',
    );
    assert(dynamicSummary, 'dynamic RGB editor');
    dynamicSummary.click();
    await clickButton('Загрузить настройки режима по датчику');
    await setField('Датчик подсветки', 'fixture:cpu');
    await setField('Нижний порог', 30);
    await setField('Верхний порог', 60);
    await clickButton('Применить режим по датчику');
    await until(async () => !!(await rpc('get_dynamic_rgb', { dev })).requested, 'dynamic enabled');
    await clickButton('Отключить режим по датчику');
    await until(async () => !(await rpc('get_dynamic_rgb', { dev })).requested, 'dynamic disabled');
    await clickButton('Применить режим по датчику');
    await until(async () => !!(await rpc('get_dynamic_rgb', { dev })).applied, 'dynamic applied');
    const dynamic = await rpc('get_dynamic_rgb', { dev });
    assert(!dynamic.last_error && !dynamic.degraded, 'dynamic live fixture input');
    // Share one source across both cooling channels, thermal and dynamic RGB.
    const fanCurve = {
      mode: 'fan_curve',
      sensor_id: 'fixture:cpu',
      points: [
        { celsius: 30, percent: 30 },
        { celsius: 70, percent: 70 },
      ],
      hysteresis_c: 0,
      fallback_percent: 95,
    };
    const committedFanCurve = (
      await rpc('set_cooling_policy', { dev, target: 'fan', policy: fanCurve })
    ).fan.requested;
    assert(
      committedFanCurve.mode === 'fan_curve' &&
        committedFanCurve.sensor_id === 'fixture:cpu' &&
        committedFanCurve.fallback_percent === 95 &&
        committedFanCurve.hysteresis_c === 0 &&
        committedFanCurve.points.length === 2 &&
        committedFanCurve.points.every(
          (p, i) =>
            p.celsius === fanCurve.points[i].celsius && p.percent === fanCurve.points[i].percent,
        ),
      'fan curve committed',
    );
    await setField('Порог CPU,', 30);
    await clickButton('Применить оповещения');
    await until(
      async () => {
        const t = await rpc('get_thermal_state', { dev });
        return (
          t.active_source === 'cpu' &&
          t.pump.applied?.kind === 'alert' &&
          t.fan.applied?.kind === 'alert'
        );
      },
      'thermal takes priority in both zones',
      200,
    );
    const alertConfig = (await rpc('get_thermal_state', { dev })).requested;
    await setField('Порог CPU,', 31);
    await setField('Нижний порог', 31);
    const consumerFault = async (mode) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: { action: 'rename', group: 'screen', slot: 0, name: `Consumer request:${mode}` },
      });
      await until(
        async () => (await screenSlot())?.name === `Consumer ready:${mode}`,
        'consumer fixture acknowledgement',
      );
    };
    await consumerFault('missing');
    await until(async () => {
      const c = await rpc('get_cooling_state', { dev }),
        t = await rpc('get_thermal_state', { dev }),
        d = await rpc('get_dynamic_rgb', { dev });
      return (
        c.pump.fallback_active &&
        c.pump.applied_percent === knownPump.fallback_percent &&
        c.fan.fallback_active &&
        c.fan.applied_percent === 95 &&
        t.active_source === 'cpu' &&
        t.degraded_sources.includes('cpu') &&
        t.pump.applied?.kind === 'alert' &&
        t.fan.applied?.kind === 'alert' &&
        d.degraded &&
        d.suppressed_by_thermal
      );
    }, 'joint fallback preserves active thermal alert');
    await until(
      () =>
        document.body.textContent.includes(
          'Подсветка временно перекрыта температурным оповещением.',
        ),
      'native GUI shows thermal ownership',
    );
    assert(
      !document.body.textContent.includes('Датчик недоступен — используется обычная подсветка.'),
      'GUI must not claim base while thermal owns LEDs',
    );
    assert(
      field('Порог CPU,').value === '31' && field('Нижний порог').value === '31',
      'combined failure preserves unsaved drafts',
    );
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).fan.requested) ===
        JSON.stringify(committedFanCurve),
      'fan fallback preserves requested curve',
    );
    assert(
      JSON.stringify((await rpc('get_thermal_state', { dev })).requested) ===
        JSON.stringify(alertConfig),
      'thermal config survives missing input',
    );
    assert(
      JSON.stringify((await rpc('get_dynamic_rgb', { dev })).requested) ===
        JSON.stringify(dynamic.requested),
      'dynamic binding survives thermal suppression',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'LCD theme survives shared source loss',
    );
    await consumerFault('restore');
    await until(async () => {
      const c = await rpc('get_cooling_state', { dev }),
        t = await rpc('get_thermal_state', { dev }),
        d = await rpc('get_dynamic_rgb', { dev });
      return (
        !c.pump.fallback_active &&
        !c.fan.fallback_active &&
        c.fan.applied_percent !== 95 &&
        t.active_source === 'cpu' &&
        !t.degraded_sources.length &&
        !d.degraded &&
        d.suppressed_by_thermal
      );
    }, 'same source resumes cooling while alert retains ownership');
    assert(
      field('Порог CPU,').value === '31' && field('Нижний порог').value === '31',
      'recovery does not apply drafts',
    );
    // Restore pre-scenario intent; raising threshold explicitly releases the alert.
    await setField('Порог CPU,', 80);
    await clickButton('Применить оповещения');
    await setField('Нижний порог', 30);
    // Actual monotonic aging: stop fixture publication, not the daemon/actors.
    await consumerFault('hot');
    await until(async () => {
      const r = (await rpc('list_sensors')).find((r) => r.id === 'fixture:cpu'),
        t = await rpc('get_thermal_state', { dev });
      return (
        r?.value === 95 &&
        r.status === 'valid' &&
        t.active_source === 'cpu' &&
        t.pump.applied?.kind === 'alert' &&
        t.fan.applied?.kind === 'alert'
      );
    }, 'hot sample triggers both thermal zones');
    const stableIntent = {
      pump: (await rpc('get_cooling_state', { dev })).pump.requested,
      fan: (await rpc('get_cooling_state', { dev })).fan.requested,
      thermal: (await rpc('get_thermal_state', { dev })).requested,
      dynamic: (await rpc('get_dynamic_rgb', { dev })).requested,
    };
    await setField('Порог CPU,', 81);
    await setField('Нижний порог', 31);
    await consumerFault('stale');
    await until(async () => {
      const r = (await rpc('list_sensors')).find((r) => r.id === 'fixture:cpu'),
        c = await rpc('get_cooling_state', { dev }),
        t = await rpc('get_thermal_state', { dev }),
        d = await rpc('get_dynamic_rgb', { dev });
      return (
        r?.status === 'stale' &&
        r.age_ms > 3000 &&
        r.value === null &&
        c.pump.fallback_active &&
        c.pump.applied_percent === knownPump.fallback_percent &&
        c.fan.fallback_active &&
        c.fan.applied_percent === 95 &&
        t.active_source === 'cpu' &&
        t.degraded_sources.includes('cpu') &&
        t.pump.applied?.kind === 'alert' &&
        t.fan.applied?.kind === 'alert' &&
        d.degraded &&
        d.suppressed_by_thermal
      );
    }, 'aged sample revoked without clearing active alert');
    await until(
      () =>
        document.body.textContent.includes(
          'Датчик недоступен; режим по датчику ожидает восстановления.',
        ),
      'GUI reflects stale source',
    );
    assert(
      field('Порог CPU,').value === '81' && field('Нижний порог').value === '31',
      'stale preserves drafts',
    );
    await consumerFault('cool');
    await until(async () => {
      const r = (await rpc('list_sensors')).find((r) => r.id === 'fixture:cpu'),
        c = await rpc('get_cooling_state', { dev }),
        t = await rpc('get_thermal_state', { dev }),
        d = await rpc('get_dynamic_rgb', { dev });
      return (
        r?.status === 'valid' &&
        r.value >= 40 &&
        r.value < 50 &&
        !c.pump.fallback_active &&
        !c.fan.fallback_active &&
        c.fan.applied_percent !== 95 &&
        t.active_source === null &&
        !t.degraded_sources.length &&
        t.fan.applied?.kind === 'base' &&
        t.pump.applied?.kind === 'base' &&
        !d.degraded &&
        !d.suppressed_by_thermal &&
        !!d.applied
      );
    }, 'fresh cool sample releases alert and restores consumers without Apply');
    const recoveredIntent = {
      pump: (await rpc('get_cooling_state', { dev })).pump.requested,
      fan: (await rpc('get_cooling_state', { dev })).fan.requested,
      thermal: (await rpc('get_thermal_state', { dev })).requested,
      dynamic: (await rpc('get_dynamic_rgb', { dev })).requested,
    };
    assert(
      JSON.stringify(recoveredIntent) === JSON.stringify(stableIntent),
      'aging and cool recovery leave all consumer configs unchanged',
    );
    assert(
      field('Порог CPU,').value === '81' && field('Нижний порог').value === '31',
      'cool recovery never applies drafts',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'LCD intent survives aged source',
    );
    await setField('Порог CPU,', 80);
    await setField('Нижний порог', 30);
    await rpc('set_cooling_policy', { dev, target: 'fan', policy: cooling.fan.requested });
    await until(async () => {
      const t = await rpc('get_thermal_state', { dev }),
        d = await rpc('get_dynamic_rgb', { dev });
      return t.active_source === null && !d.suppressed_by_thermal && !d.degraded && !!d.applied;
    }, 'dynamic RGB resumes after thermal release');
    // Selective HID short write: pump RGB fails while LCD and fan PWM keep progressing.
    await consumerFault('rgb-fail');
    await until(
      async () => (await rpc('get_dynamic_rgb', { dev })).last_error,
      'RGB transport failure visible',
    );
    const failedRgb = await rpc('get_dynamic_rgb', { dev });
    assert(
      JSON.stringify(failedRgb.requested) === JSON.stringify(dynamic.requested),
      'write failure preserves RGB intent',
    );
    await rpc('set_cooling_policy', { dev, target: 'fan', policy: { mode: 'fixed', percent: 61 } });
    await until(async () => {
      const c = await rpc('get_cooling_state', { dev });
      return c.fan.applied_percent === 61 && !c.fan.last_error && !c.pump.last_error;
    }, 'cooling applies while RGB write fails');
    await delay(2500); // Observe multiple 1 Hz LCD frames during the held fault.
    assert(
      (await rpc('get_dynamic_rgb', { dev })).last_error,
      'fault remains active during independent work',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'RGB failure preserves LCD theme',
    );
    await consumerFault('rgb-restore');
    await until(async () => {
      const d = await rpc('get_dynamic_rgb', { dev });
      return !d.last_error && !d.degraded && !d.suppressed_by_thermal && !!d.applied;
    }, 'RGB automatically recovers after transport resumes');
    assert(
      JSON.stringify((await rpc('get_dynamic_rgb', { dev })).requested) ===
        JSON.stringify(dynamic.requested),
      'RGB recovery retains binding',
    );
    await rpc('set_cooling_policy', { dev, target: 'fan', policy: cooling.fan.requested });
    await until(
      async () => (await rpc('get_cooling_state', { dev })).fan.applied_percent === 55,
      'restore fan intent after transport scenario',
    );
    // A failed theme frame stops that producer, preserving saved intent for explicit recovery.
    await consumerFault('lcd-fail');
    await until(async () => {
      const l = await rpc('get_lcd_state', { dev });
      return l.source === 'Idle' && !!l.last_error;
    }, 'LCD failed frame stops with visible error');
    await rpc('set_cooling_policy', { dev, target: 'fan', policy: { mode: 'fixed', percent: 62 } });
    await until(async () => {
      const c = await rpc('get_cooling_state', { dev });
      return c.fan.applied_percent === 62 && !c.fan.last_error && !c.pump.last_error;
    }, 'cooling continues during LCD fault');
    await delay(2500);
    assert(!(await rpc('get_dynamic_rgb', { dev })).last_error, 'RGB unaffected by LCD failure');
    assert(
      (await rpc('get_lcd_theme', { dev })) === null,
      'stopped producer does not claim an active theme',
    );
    await consumerFault('lcd-restore');
    assert(
      (await rpc('get_lcd_state', { dev })).source === 'Idle',
      'LCD recovery requires explicit Apply after stopped producer',
    );
    await rpc('set_lcd_theme', { dev, theme: stored });
    await until(async () => {
      const l = await rpc('get_lcd_state', { dev });
      return l.source === 'Theme' && !l.last_error;
    }, 'explicit LCD recovery resumes theme');
    // Failure of an already active fan curve revokes applied state but retains its intent.
    await rpc('set_cooling_policy', { dev, target: 'fan', policy: fanCurve });
    await consumerFault('pwm-fail');
    await until(async () => {
      const c = await rpc('get_cooling_state', { dev });
      return !!c.fan.last_error && c.fan.applied_percent === null && !c.pump.last_error;
    }, 'fan failure revokes applied PWM, pump remains healthy');
    await delay(2500);
    const fanFailed = await rpc('get_cooling_state', { dev }),
      lcdDuringFan = await rpc('get_lcd_state', { dev });
    assert(
      JSON.stringify(fanFailed.fan.requested) === JSON.stringify(committedFanCurve),
      'PWM failure retains curve',
    );
    assert(
      lcdDuringFan.source === 'Theme' &&
        !lcdDuringFan.last_error &&
        !(await rpc('get_dynamic_rgb', { dev })).last_error,
      'LCD and RGB unaffected by PWM failure',
    );
    await consumerFault('pwm-restore');
    await until(async () => {
      const c = await rpc('get_cooling_state', { dev });
      return !c.fan.last_error && c.fan.applied_percent !== null && !c.pump.last_error;
    }, 'fan curve automatically recovers without Apply');
    assert(
      JSON.stringify((await rpc('get_cooling_state', { dev })).fan.requested) ===
        JSON.stringify(committedFanCurve),
      'PWM recovery retains curve binding',
    );
    await rpc('set_cooling_policy', { dev, target: 'fan', policy: cooling.fan.requested });
    await until(
      async () => (await rpc('get_cooling_state', { dev })).fan.applied_percent === 55,
      'restore fan55 after LCD/PWM fault cases',
    );
    // The Python harness observes this profile marker and restarts only its fixture.
    await clickButton('Экран помпы');
    await setField('Название', 'Unsaved across native reconnect');
    const reconnectLibrary = await rpc('list_profiles');
    await rpc('edit_profile', {
      expected_revision: reconnectLibrary.revision,
      edit: { action: 'rename', group: 'screen', slot: 0, name: 'Native reconnect ready' },
    });
    await until(
      () => document.querySelector('.connection')?.textContent.includes('Нет связи с daemon'),
      'native disconnect visible',
    );
    assert(
      button('Применить к LCD').disabled && button('Загрузить активную тему').disabled,
      'offline device actions disabled',
    );
    assert(
      field('Название').value === 'Unsaved across native reconnect',
      'disconnect preserves theme draft',
    );
    await until(
      () => document.querySelector('.connection')?.textContent.includes('Daemon подключён'),
      'native reconnect visible',
    );
    await until(() => !button('Применить к LCD').disabled, 'device available after reconnect');
    assert(
      field('Название').value === 'Unsaved across native reconnect',
      'reconnect preserves theme draft',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'reconnect replays saved theme without applying draft',
    );
    // Explicit load and Apply must work through the reconnected bridge.
    await clickButton('Загрузить активную тему');
    await until(() => field('Название')?.value === stored.name, 'load after native reconnect');
    await clickButton('Применить к LCD');
    await until(
      () => document.querySelector('.notice[role=status]')?.textContent.includes('Тема отправлена'),
      'Apply after native reconnect',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'post-reconnect Apply retains full theme',
    );
    const reconnect = async (pageName, offline, online) => {
      const library = await rpc('list_profiles');
      await rpc('edit_profile', {
        expected_revision: library.revision,
        edit: { action: 'rename', group: 'screen', slot: 0, name: `Native reconnect ${pageName}` },
      });
      await until(
        () => document.querySelector('.connection')?.textContent.includes('Нет связи с daemon'),
        `${pageName} offline`,
      );
      await offline();
      await until(
        () => document.querySelector('.connection')?.textContent.includes('Daemon подключён'),
        `${pageName} online`,
      );
      await online();
    };
    await clickButton('Охлаждение');
    await setField('Канал', 'fan');
    await setField('PWM, %', 56);
    await reconnect(
      'cooling',
      () => {
        assert(button('Применить охлаждение').disabled, 'cooling offline Apply disabled');
        assert(field('PWM, %').value === '56', 'cooling offline draft');
      },
      async () => {
        await until(() => !button('Применить охлаждение').disabled, 'cooling online Apply');
        assert(field('PWM, %').value === '56', 'cooling reconnect draft');
        assert(
          (await rpc('get_cooling_state', { dev })).fan.requested.percent === 55,
          'cooling draft not auto-applied',
        );
      },
    );
    await clickButton('Настройки');
    await clickButton('Загрузить текущие значения');
    await setField('Яркость LCD:', 64);
    await setField('Ориентация', 90);
    await reconnect(
      'settings',
      () => {
        assert(
          button('Применить яркость').disabled && button('Применить поворот').disabled,
          'settings offline Apply disabled',
        );
        assert(
          field('Яркость LCD:').value === '64' && field('Ориентация').value === '90',
          'settings offline draft',
        );
      },
      async () => {
        await until(() => !button('Применить яркость').disabled, 'settings online Apply');
        assert(
          field('Яркость LCD:').value === '64' && field('Ориентация').value === '90',
          'settings reconnect draft',
        );
        assert(
          JSON.stringify((await rpc('get_lcd_state', { dev })).requested) ===
            JSON.stringify(lcd.requested),
          'settings draft not auto-applied',
        );
      },
    );
    await clickButton('Подсветка');
    await clickButton('Загрузить текущие оповещения');
    await setField('Порог CPU,', 81);
    await setField('Цвет RGB', '#123456');
    const reopenedDynamic = Array.from(document.querySelectorAll('summary')).find(
      (s) => s.textContent.trim() === 'Подсветка помпы по датчику',
    );
    if (!reopenedDynamic.parentElement.open) reopenedDynamic.click();
    await clickButton('Загрузить настройки режима по датчику');
    await setField('Верхний порог', 61);
    await reconnect(
      'lighting',
      () => {
        for (const name of [
          'Применить подсветку',
          'Применить оповещения',
          'Применить режим по датчику',
          'Отключить режим по датчику',
        ])
          assert(button(name).disabled, `${name} offline disabled`);
        assert(
          field('Порог CPU,').value === '81' &&
            field('Верхний порог').value === '61' &&
            field('Цвет RGB').value === '#123456',
          'lighting offline drafts',
        );
      },
      async () => {
        await until(() => !button('Применить режим по датчику').disabled, 'dynamic online Apply');
        assert(
          field('Порог CPU,').value === '81' &&
            field('Верхний порог').value === '61' &&
            field('Цвет RGB').value === '#123456',
          'lighting reconnect drafts',
        );
        assert(
          JSON.stringify((await rpc('get_thermal_state', { dev })).requested) ===
            JSON.stringify(thermal.requested),
          'thermal draft not auto-applied',
        );
        assert(
          JSON.stringify((await rpc('get_dynamic_rgb', { dev })).requested) ===
            JSON.stringify(dynamic.requested),
          'dynamic draft not auto-applied',
        );
      },
    );
    await clickButton('Датчики');
    const cpuCard = () =>
      Array.from(document.querySelectorAll('.sensors article')).find(
        (el) => el.querySelector('h2')?.textContent === 'Fixture CPU',
      );
    await until(
      () => cpuCard()?.querySelector('strong')?.textContent.includes('°C'),
      'CPU reading before disconnect',
    );
    await reconnect(
      'sensors',
      () => {
        assert(
          field('Системная видеокарта').value === '',
          'source draft survives navigation and disconnect',
        );
        assert(
          button('Применить источники').disabled && button('Загрузить сохранённый выбор').disabled,
          'source writes/reload disabled offline',
        );
        assert(
          Array.from(sourceEditor().querySelectorAll('dd')).every((e) => e.textContent === '—'),
          'source values revoked offline',
        );
        assert(
          cpuCard()?.querySelector('strong')?.textContent === '—',
          'offline sensor is not stale value or fake zero',
        );
        assert(
          cpuCard()?.querySelector('p')?.textContent.includes('unavailable'),
          'offline sensor explicit status',
        );
      },
      async () => {
        await until(
          () => cpuCard()?.querySelector('strong')?.textContent.includes('°C'),
          'CPU reading after reconnect',
        );
        assert(
          cpuCard()?.querySelector('code')?.textContent === 'fixture:cpu',
          'sensor identity preserved',
        );
        assert(
          cpuCard()?.querySelector('p')?.textContent.startsWith('valid'),
          'fresh valid sensor status',
        );
        await until(
          () => !button('Загрузить сохранённый выбор').disabled,
          'source fresh read after reconnect',
        );
        assert(
          field('Системная видеокарта').value === '',
          'source reconnect does not overwrite clear draft',
        );
        assert(
          JSON.stringify((await rpc('get_sensor_defaults')).config) ===
            JSON.stringify(sensorDefaults),
          'source config persists across fixture restart',
        );
        await clickButton('Загрузить сохранённый выбор');
        assert(
          field('Системная видеокарта').value === sensorDefaults.gpu.device_id,
          'explicit Intel source reload',
        );
        assert(
          field('GPU · Загрузка').value === sensorDefaults.gpu.load,
          'Intel engine binding restored',
        );
        assert(
          field('GPU · Использование VRAM').value === sensorDefaults.gpu.memory_used,
          'Intel VRAM binding restored',
        );
      },
    );
    await clickButton('Настройки');
    await clickButton('Загрузить текущие значения');
    await setField('Яркость LCD:', 66);
    await clickButton('Применить яркость');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
          el.textContent.includes('response dropped'),
        ),
      'lost response error visible',
    );
    assert(field('Яркость LCD:').value === '66', 'ambiguous mutation preserves draft');
    assert(
      !Array.from(document.querySelectorAll('[role=status]')).some((el) =>
        el.textContent.includes('Настройка отправлена'),
      ),
      'lost reply not reported as success',
    );
    await until(async () => {
      try {
        return (await rpc('get_lcd_state', { dev })).requested.brightness_percent === 66;
      } catch {
        return false;
      }
    }, 'read actual state after lost reply');
    // The proxy counts this mutation: read recovery must not retry a write.
    await until(
      () =>
        document
          .querySelector('section[aria-label="Настройки LCD"]')
          ?.textContent.includes('66% · 180°'),
      'GUI polls actual state after lost reply',
    );
    await setField('Яркость LCD:', 65);
    await clickButton('Применить яркость');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=status]')).some((el) =>
          el.textContent.includes('Настройка отправлена'),
        ),
      'explicit retry recovers after lost reply',
    );
    assert(
      (await rpc('get_lcd_state', { dev })).requested.brightness_percent === 65,
      'explicit restore after lost reply',
    );
    await setField('Яркость LCD:', 67);
    const timeoutStarted = performance.now();
    await clickButton('Применить яркость');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
          el.textContent.includes('Request timed out; applied state must be checked'),
        ),
      'ambiguous write timeout visible',
      150,
    );
    assert(performance.now() - timeoutStarted >= 9500, 'real bridge deadline exercised');
    assert(field('Яркость LCD:').value === '67', 'timeout preserves draft');
    assert(
      !Array.from(document.querySelectorAll('[role=status]')).some((el) =>
        el.textContent.includes('Настройка отправлена'),
      ),
      'timeout not reported as success',
    );
    assert(
      (await rpc('get_lcd_state', { dev })).requested.brightness_percent === 67,
      'timeout does not imply write rollback',
    );
    await until(
      () =>
        document
          .querySelector('section[aria-label="Настройки LCD"]')
          ?.textContent.includes('67% · 180°'),
      'GUI readback after timeout',
    );
    await setField('Яркость LCD:', 65);
    await clickButton('Применить яркость');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=status]')).some((el) =>
          el.textContent.includes('Настройка отправлена'),
        ),
      'explicit Apply after timeout',
    );
    assert(
      (await rpc('get_lcd_state', { dev })).requested.brightness_percent === 65,
      'timeout recovery restores brightness',
    );
    await clickButton('Экран помпы');
    await clickButton('Загрузить активную тему');
    await until(
      () => field('Название')?.value === stored.name,
      'load theme before ambiguous replacement',
    );
    const beforeThemeLibrary = await rpc('list_profiles');
    await setField('Название', 'Native lost theme reply');
    await setField('Первый цвет фона', '#e01234');
    const replacement = structuredClone(stored);
    replacement.name = 'Native lost theme reply';
    replacement.background_fill.radial.colors[0] = [224, 18, 52];
    await clickButton('Применить к LCD');
    await setField('Название', 'Native unsent theme edits');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
          el.textContent.includes('response dropped'),
        ),
      'theme lost reply error',
    );
    assert(
      !Array.from(document.querySelectorAll('[role=status]')).some((el) =>
        el.textContent.includes('Тема отправлена.'),
      ),
      'theme lost reply not success',
    );
    assert(
      field('Название').value === 'Native unsent theme edits',
      'theme loss preserves later draft',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(replacement),
      'replacement committed despite lost reply',
    );
    assert(
      JSON.stringify(await rpc('list_profiles')) === JSON.stringify(beforeThemeLibrary),
      'theme replacement does not mutate profiles',
    );
    await clickButton('Загрузить активную тему');
    await until(
      () => field('Название')?.value === replacement.name,
      'explicit readback recovers replacement',
    );
    assert(
      field('Первый цвет фона').value === '#e01234',
      'native readback recovers changed background',
    );
    await setField('Название', 'Draft survives clear');
    await clickButton('Вернуть LocalUI');
    await until(
      () =>
        Array.from(document.querySelectorAll('[role=alert]')).some((el) =>
          el.textContent.includes('response dropped'),
        ),
      'clear lost reply error',
    );
    assert(field('Название').value === 'Draft survives clear', 'clear loss preserves draft');
    assert(
      !Array.from(document.querySelectorAll('[role=status]')).some((el) =>
        el.textContent.includes('Запрошен LocalUI.'),
      ),
      'clear loss not success',
    );
    assert((await rpc('get_lcd_theme', { dev })) === null, 'clear committed despite lost reply');
    await until(
      async () => (await rpc('get_lcd_state', { dev })).source === 'Idle',
      'clear idle readback',
    );
    assert(
      JSON.stringify(await rpc('list_profiles')) === JSON.stringify(beforeThemeLibrary),
      'clear does not mutate profile library',
    );
    await setField('Название', stored.name);
    const originalColor =
      '#' +
      stored.background_fill.radial.colors[0].map((v) => v.toString(16).padStart(2, '0')).join('');
    await setField('Первый цвет фона', originalColor);
    await clickButton('Применить к LCD');
    await until(
      async () => JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(stored),
      'explicit restore after ambiguous clear',
    );
    const resumedLibrary = await rpc('list_profiles');
    await rpc('edit_profile', {
      expected_revision: resumedLibrary.revision,
      edit: { action: 'rename', group: 'screen', slot: 0, name: 'Native retained screen' },
    });
    const profiles = await rpc('list_profiles');
    assert(
      profiles.profiles.length === 5 &&
        new Set(profiles.profiles.map((p) => p.content.kind)).size === 3,
      'three groups plus retained profile and fault slots',
    );
    assert(
      profiles.profiles.some(
        (p) => p.slot === 7 && p.name === 'Native fault slot' && p.content.kind === 'screen',
      ),
      'pre-forward failure retained original test slot',
    );
    const controls = {
      pump: cooling.pump.requested,
      fan: cooling.fan.requested,
      lcd: lcd.requested,
      thermal: thermal.requested,
      dynamic: dynamic.requested,
      base_mode: dynamic.base_mode,
    };
    await invoke('native_smoke_result', {
      result: {
        ok: true,
        dev,
        asset,
        preview_bytes: preview.length,
        theme: stored,
        controls,
        profiles,
        native_cooling_modes: !!nativeCoolingModes,
        native_cooling_defaults: !!window.__nativeCoolingDefaults,
        cooling_mode_banks: nativeCoolingModes,
        sensor_defaults: sensorDefaults,
        native_sensor_defaults: true,
        native_intel_sources: true,
        native_recommended_defaults: true,
        native_cpu_fan_mapping: true,
        native_gradient_background: true,
        native_font_import: true,
        native_display_fonts: true,
        native_noto_coverage: true,
        native_text_fallback: true,
        native_font_retry: true,
        native_font_partial_upload: true,
        native_saved_dependency_recovery: true,
        native_restart_dependency_recovery: true,
        native_active_sensor_recovery: true,
        native_combined_consumers: true,
        native_aged_sensor_recovery: true,
        native_rgb_transport_recovery: true,
        native_lcd_pwm_transport_recovery: true,
        native_font_parts: true,
        native_font_cleanup: true,
        native_font_retention: true,
        native_draft_session_retention: true,
        native_image_cleanup: true,
        native_cleanup_corrupt_root: true,
        native_cleanup_corrupt_profiles: true,
        native_cleanup_partial_retry: true,
        native_source_lost_reply: true,
        native_theme_lost_reply: true,
        native_clear_lost_reply: true,
        native_profile_lost_reply: true,
        native_profile_extra_faults: true,
        profile_fault_recovery: profileFaultRecovery,
        profile_conflict_recovery: true,
        missing_asset_rejection: true,
        missing_sensor_profile_recovery: true,
        native_display_reconnect: true,
        native_controls_reconnect: true,
        native_lost_reply: true,
        native_write_timeout: true,
      },
    });
  } catch (e) {
    await invoke('native_smoke_result', {
      result: {
        ok: false,
        error: e?.message ?? String(e),
        detail: JSON.stringify(e),
        body: document.body.innerText,
        alerts: Array.from(document.querySelectorAll('[role=alert]')).map((el) => el.textContent),
        stack: e?.stack,
      },
    });
  }
})();
