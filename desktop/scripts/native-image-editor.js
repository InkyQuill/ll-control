(async () => {
  if (window.__nativeImageEditorStarted) return;
  window.__nativeImageEditorStarted = true;
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const assert = (ok, why) => {
    if (!ok) throw new Error(why);
  };
  const wait = async (predicate, why) => {
    for (let i = 0; i < 600; i++) {
      if (await predicate()) return;
      await delay(50);
    }
    throw new Error(why);
  };
  const equal = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
  let phase = 'setup',
    keySequence = 0,
    trustedInputs = 0;
  const pointerEvents = [],
    releaseEvents = [];
  for (const type of ['pointerup', 'pointercancel', 'mouseup'])
    document.addEventListener(
      type,
      (event) => {
        if (phase === 'trusted-crop-input')
          releaseEvents.push({ type, trusted: event.isTrusted, target: event.target?.tagName });
      },
      true,
    );
  try {
    const settings = await rpc('get_display_settings');
    const text = (ru, en) => (settings.interface_language === 'ru-RU' ? ru : en);
    const buttons = (name) =>
      [...document.querySelectorAll('button')].filter(
        (b) =>
          (b.getAttribute('aria-label') || b.textContent.trim()) === name &&
          b.getClientRects().length &&
          !b.closest('[hidden]'),
      );
    const button = (name) => buttons(name)[0];
    const click = async (name) => {
      await wait(() => button(name) && !button(name).disabled, name);
      button(name).focus();
      button(name).click();
      await delay(0);
    };
    const editor = () =>
      document.querySelector(
        `section[aria-label="${text('Редактор изображения', 'Image editor')}"]`,
      );
    const field = (name) =>
      [...editor().querySelectorAll('label')]
        .find((label) => label.textContent.trim() === name)
        ?.querySelector('input');
    document.addEventListener(
      'input',
      (event) => {
        if (event.isTrusted && editor()?.contains(event.target)) trustedInputs++;
      },
      true,
    );
    async function key(value, probe = null) {
      const expected = { space: ' ', Home: 'Home', Tab: 'Tab', Return: 'Enter' }[value];
      let listener, timer;
      const received = new Promise((resolve, reject) => {
        listener = (event) => {
          if (event.isTrusted && event.key === expected) resolve();
        };
        document.addEventListener('keyup', listener, true);
        timer = setTimeout(() => reject(new Error('missing trusted keyup ' + value)), 10000);
      });
      try {
        await invoke('native_keyboard_step', { sequence: ++keySequence, key: value, probe });
        await received;
      } finally {
        clearTimeout(timer);
        document.removeEventListener('keyup', listener, true);
      }
      await delay(50);
    }
    const set = (name, value) => {
      const input = field(name);
      assert(input && !input.disabled, 'enabled field ' + name);
      input.value = String(value);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    };
    await wait(async () => (await rpc('list_devices')).length === 1, 'one mock LCDV2');
    const dev = (await rpc('list_devices'))[0].id;
    const sourceCanvas = document.createElement('canvas');
    sourceCanvas.width = 420;
    sourceCanvas.height = 280;
    const ctx = sourceCanvas.getContext('2d');
    const pixels = ctx.createImageData(420, 280);
    for (let y = 0; y < 280; y++)
      for (let x = 0; x < 420; x++) {
        const i = (y * 420 + x) * 4;
        pixels.data.set([x % 256, y % 256, (x + y) % 256, 64 + ((x + y) % 192)], i);
      }
    ctx.putImageData(pixels, 0, 0);
    const blob = await new Promise((resolve) => sourceCanvas.toBlob(resolve, 'image/png'));
    const original = await invoke('import_image', {
      bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
      name: 'Native gallery.png',
    });
    const sourceBytes = await rpc('read_theme_asset_for_edit', { key: original });
    await click(text('Экран помпы', 'Pump screen'));
    const sourceStatus = () => document.querySelector('.visual .status > span')?.textContent.trim();
    const idleLabel = text('Источник: Нет программного потока', 'Source: No software stream');
    const themeLabel = text('Источник: Тема LCD', 'Source: LCD theme');
    await wait(() => sourceStatus() === idleLabel, 'localized idle source from native readback');
    await click(text('Композиция', 'Composition'));
    while (button(text('Удалить слой', 'Remove layer')))
      await click(text('Удалить слой', 'Remove layer'));
    await click(text('Библиотека', 'Library'));
    const summary = [...document.querySelectorAll('summary')].find(
      (s) => s.textContent.trim() === text('Библиотека изображений', 'Image library'),
    );
    assert(summary, 'image gallery summary');
    if (!summary.parentElement.open) summary.click();
    await click(
      text(`Добавить в тему ${original.slice(0, 8)}`, `Add ${original.slice(0, 8)} to theme`),
    );
    await click(text('Применить к LCD', 'Apply to LCD'));
    await wait(
      async () => (await rpc('get_lcd_theme', { dev }))?.layers.some((l) => l.asset === original),
      'initial saved theme reference',
    );
    const oldTheme = await rpc('get_lcd_theme', { dev });
    await wait(
      () => sourceStatus() === themeLabel,
      'localized active theme source from native readback',
    );
    assert(
      oldTheme.layers.length === 1 && oldTheme.layers[0].kind === 'image',
      'static single-image baseline, no clock drift',
    );
    await click(text('Библиотека', 'Library'));
    phase = 'open-source';
    await click(text('Редактировать Native gallery.png…', 'Edit Native gallery.png…'));
    await wait(
      () =>
        editor()?.querySelector('img')?.complete && field(text('Обрезать квадрат', 'Crop square')),
      'source opened in editor',
    );
    const crop = field(text('Обрезать квадрат', 'Crop square'));
    phase = 'trusted-crop-input';
    assert(!crop.checked, 'gallery edit starts without implicit crop');
    crop.focus();
    await key('space');
    assert(crop.checked, 'trusted Space enables crop');
    for (const [name, probe, value] of [
      ['Crop X', 'image_crop_x', 30],
      ['Crop Y', 'image_crop_y', 20],
      [text('Сторона crop', 'Crop side length'), 'image_crop_side', 150],
    ]) {
      await key('Tab');
      assert(document.activeElement === field(name), 'native Tab reaches ' + name);
      await key('Home', probe);
      assert(Number(field(name).value) === value, 'native typing updates ' + name);
    }
    assert(trustedInputs >= 7, 'numeric edits delivered trusted input events');
    const sourceButton = editor().querySelector('button.source');
    for (const type of [
      'pointerdown',
      'pointermove',
      'pointerup',
      'pointercancel',
      'lostpointercapture',
    ])
      sourceButton.addEventListener(type, (event) => {
        if (event.isTrusted) {
          const bounds = sourceButton.getBoundingClientRect();
          pointerEvents.push({
            type,
            buttons: event.buttons,
            x: event.clientX,
            y: event.clientY,
            bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height },
            outside: event.clientX > bounds.right || event.clientY > bounds.bottom,
          });
        }
      });
    for (const type of ['dragstart', 'dragend'])
      sourceButton.addEventListener(type, (event) =>
        pointerEvents.push({ type, trusted: event.isTrusted, target: event.target?.tagName }),
      );
    sourceButton.scrollIntoView({ block: 'center' });
    sourceButton.focus();
    await key('space', 'image_crop_pointer');
    await wait(
      () => Number(field('Crop X').value) === 270 && Number(field('Crop Y').value) === 130,
      'crop stays clamped after captured drag and unpressed pointer return',
    );
    assert(
      pointerEvents.some((e) => e.type === 'pointerdown') &&
        pointerEvents.some((e) => e.type === 'pointermove' && e.outside),
      'trusted pointer capture delivers outside movement',
    );
    const outsideIndex = pointerEvents.findLastIndex(
      (e) => e.type === 'pointermove' && e.outside && e.buttons === 1,
    );
    assert(
      pointerEvents
        .slice(outsideIndex + 1)
        .some((e) => e.type === 'pointermove' && e.buttons === 0 && !e.outside),
      'unpressed pointer returned inside without moving crop',
    );
    assert(
      !pointerEvents.some((e) => e.type === 'dragstart'),
      'crop must not initiate native image drag-and-drop',
    );
    const actions = [
      ['Повернуть вправо', 'Rotate right', 'rotate_right'],
      ['Отразить по горизонтали', 'Flip horizontally', 'flip_horizontal'],
      ['Повернуть влево', 'Rotate left', 'rotate_left'],
      ['Отразить по вертикали', 'Flip vertically', 'flip_vertical'],
    ];
    const transforms = [];
    for (let i = 0; i < 64; i++) {
      const [ru, en, value] = actions[i % 4];
      await click(text(ru, en));
      transforms.push(value);
    }
    assert(button(text('Повернуть вправо', 'Rotate right')).disabled, '64 action UI limit');
    await click(text('Отменить преобразование', 'Undo transformation'));
    transforms.pop();
    assert(
      !button(text('Повернуть вправо', 'Rotate right')).disabled,
      'undo restores action admission',
    );
    for (const [ru, en, value] of [
      ['Сдвиг тона', 'Hue shift', 27],
      ['Насыщенность', 'Saturation', 12],
      ['Яркость изображения', 'Image brightness', 5],
      ['Контраст изображения', 'Image contrast', 10],
      ['Резкость', 'Sharpen', 5],
      ['Размытие', 'Blur', 2.5],
    ])
      set(text(ru, en), value);
    phase = 'fixed-engine-request';
    const expected = await invoke('image_editor', {
      bytes: sourceBytes,
      edit: {
        crop: { x: 270, y: 130, side: 150 },
        transforms,
        filters: { hue: 27, saturation: 12, brightness: 5, contrast: 10, sharpen: 5, blur: 2.5 },
      },
    });
    const catalogBefore = await rpc('list_named_theme_assets');
    phase = 'edited-preview';
    await click(text('Подготовить preview изображения', 'Prepare image preview'));
    const preview = () => editor()?.querySelector('img.preview');
    await wait(
      () => preview()?.complete && preview().naturalWidth === 480,
      'native edited preview decoded',
    );
    const expectedImage = new Image();
    const expectedUrl = URL.createObjectURL(
      new Blob([new Uint8Array(expected.png)], { type: 'image/png' }),
    );
    try {
      expectedImage.src = expectedUrl;
      await expectedImage.decode();
      const decoded = (img) => {
        const canvas = document.createElement('canvas');
        canvas.width = canvas.height = 480;
        const context = canvas.getContext('2d');
        context.drawImage(img, 0, 0);
        return context.getImageData(0, 0, 480, 480).data;
      };
      assert(
        equal(decoded(preview()), decoded(expectedImage)),
        'UI crop/actions/filter pixels match fixed engine request',
      );
    } finally {
      URL.revokeObjectURL(expectedUrl);
    }
    assert(
      (await rpc('list_named_theme_assets')).length === catalogBefore.length,
      'preview never publishes',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(oldTheme),
      'preview preserves active theme',
    );
    phase = 'save';
    button(text('Сохранить изображение в галерею', 'Save image to gallery')).focus();
    await key('Return');
    let replacement;
    await wait(async () => {
      const state = await rpc('get_screen_collections', { dev });
      replacement = state.collections?.images.find(
        (k) => k !== original && !catalogBefore.some((a) => a.key === k),
      );
      return replacement && !state.persistence.pending;
    }, 'new asset and checked collection replacement');
    assert(
      equal(await rpc('read_theme_asset_for_edit', { key: original }), sourceBytes),
      'original bytes preserved',
    );
    assert(
      equal(await rpc('read_theme_asset_for_edit', { key: replacement }), expected.png),
      'saved derivative matches preview',
    );
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(oldTheme),
      'Save preserves active old reference',
    );
    await wait(() => !editor(), 'replaced card releases editor');
    await wait(() => document.activeElement === summary, 'replaced card restores summary focus');
    await wait(
      () =>
        [...document.querySelectorAll('img[alt="Native gallery.png"]')].some(
          (img) => img.complete && img.naturalWidth <= 144,
        ),
      'new gallery thumbnail decoded',
    );
    phase = 'reopen-and-cancel';
    const editButton = text('Редактировать Native gallery.png…', 'Edit Native gallery.png…');
    await click(editButton);
    await wait(() => editor()?.querySelector('img')?.complete, 'derivative can be reopened');
    const beforeCancel = await rpc('get_screen_collections', { dev });
    await click(text('Повернуть вправо', 'Rotate right'));
    await click(text('Подготовить preview изображения', 'Prepare image preview'));
    await wait(
      () => preview()?.complete && preview().naturalWidth === 480,
      'cancelled edit preview decoded',
    );
    await click(text('Закрыть редактор изображения', 'Close image editor'));
    assert(
      JSON.stringify(await rpc('get_screen_collections', { dev })) === JSON.stringify(beforeCancel),
      'Close without Save preserves membership',
    );
    assert(
      (await rpc('list_named_theme_assets')).length === catalogBefore.length + 1,
      'Close never publishes a second derivative',
    );
    await wait(
      () => document.activeElement === button(editButton),
      'Close restores edit trigger focus',
    );
    phase = 'explicit-apply';
    await click(text('Обновить preview', 'Refresh preview'));
    await wait(() => document.querySelector('.lcd img')?.complete, 'theme preview decoded');
    assert(
      JSON.stringify(await rpc('get_lcd_theme', { dev })) === JSON.stringify(oldTheme),
      'draft theme preview preserves old active reference',
    );
    await click(text('Применить к LCD', 'Apply to LCD'));
    await wait(
      async () =>
        (await rpc('get_lcd_theme', { dev }))?.layers.some((l) => l.asset === replacement),
      'explicit Apply changes active reference',
    );
    await invoke('native_media_step', { sequence: 1, action: 'editor_saved' });
    await click(text('Вернуть LocalUI', 'Restore LocalUI'));
    await wait(
      async () => (await rpc('get_lcd_state', { dev })).source === 'Idle',
      'final LocalUI',
    );
    await wait(() => sourceStatus() === idleLabel, 'localized idle source after explicit LocalUI');
    phase = 'exif-normalization';
    const decodePixels = async (bytes) => {
      const image = new Image(),
        url = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
      try {
        image.src = url;
        await image.decode();
        const canvas = document.createElement('canvas');
        canvas.width = image.naturalWidth;
        canvas.height = image.naturalHeight;
        const context = canvas.getContext('2d');
        context.drawImage(image, 0, 0);
        return {
          width: canvas.width,
          height: canvas.height,
          data: context.getImageData(0, 0, canvas.width, canvas.height).data,
        };
      } finally {
        URL.revokeObjectURL(url);
      }
    };
    const exifCanvas = document.createElement('canvas');
    exifCanvas.width = 32;
    exifCanvas.height = 16;
    const exifContext = exifCanvas.getContext('2d');
    for (let y = 0; y < 16; y++)
      for (let x = 0; x < 32; x++) {
        exifContext.fillStyle = `rgb(${x * 7},${y * 13},${(x + y) * 3})`;
        exifContext.fillRect(x, y, 1, 1);
      }
    const jpeg = Array.from(
      new Uint8Array(
        await (
          await new Promise((resolve) => exifCanvas.toBlob(resolve, 'image/jpeg', 0.95))
        ).arrayBuffer(),
      ),
    );
    const baseline = await invoke('image_editor', { bytes: jpeg, edit: null });
    const raw = await decodePixels(baseline.png),
      exifCases = [];
    for (let orientation = 1; orientation <= 8; orientation++) {
      const exif = [
        69,
        120,
        105,
        102,
        0,
        0,
        73,
        73,
        42,
        0,
        8,
        0,
        0,
        0,
        1,
        0,
        18,
        1,
        3,
        0,
        1,
        0,
        0,
        0,
        orientation,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
      ];
      const length = exif.length + 2,
        bytes = [
          ...jpeg.slice(0, 2),
          255,
          225,
          length >> 8,
          length & 255,
          ...exif,
          ...jpeg.slice(2),
        ];
      const prepared = await invoke('image_editor', { bytes, edit: null });
      const key = await invoke('import_image', { bytes, name: `EXIF ${orientation}.jpg` });
      const normalized = await rpc('read_theme_asset_for_edit', { key });
      const actual = await decodePixels(normalized);
      assert(
        actual.width === (orientation >= 5 ? 16 : 32) &&
          actual.height === (orientation >= 5 ? 32 : 16),
        'oriented native dimensions ' + orientation,
      );
      for (let y = 0; y < actual.height; y++)
        for (let x = 0; x < actual.width; x++) {
          const [sx, sy] = [
            [x, y],
            [31 - x, y],
            [31 - x, 15 - y],
            [x, 15 - y],
            [y, x],
            [y, 15 - x],
            [31 - y, 15 - x],
            [31 - y, x],
          ][orientation - 1];
          const a = (y * actual.width + x) * 4,
            b = (sy * raw.width + sx) * 4;
          assert(
            equal(actual.data.slice(a, a + 4), raw.data.slice(b, b + 4)),
            'EXIF native pixel mapping ' + orientation,
          );
        }
      const preview = await decodePixels(prepared.png);
      assert(
        preview.width === actual.width &&
          preview.height === actual.height &&
          equal(preview.data, actual.data),
        'editor/import orientation agreement',
      );
      const reread = await invoke('image_editor', { bytes: normalized, edit: null });
      const again = await decodePixels(reread.png);
      assert(
        again.width === actual.width &&
          again.height === actual.height &&
          equal(again.data, actual.data),
        'normalized source must not rotate twice',
      );
      exifCases.push({ orientation, key, width: actual.width, height: actual.height });
    }
    await invoke('native_smoke_result', {
      keepOpen: true,
      result: {
        ok: true,
        language: settings.interface_language,
        exif_cases: exifCases,
        source_labels: { idle: idleLabel, theme: themeLabel },
        trusted_key_steps: keySequence,
        trusted_inputs: trustedInputs,
        trusted_pointer_events: pointerEvents,
        release_events: releaseEvents,
        keyboard_save: true,
        typed_crop_and_pointer_clamp: true,
        original,
        replacement,
        crop_and_63_actions: true,
        all_filters: true,
        native_preview_decode: true,
        exact_engine_request_mapping: true,
        source_unchanged: true,
        save_requires_apply: true,
        old_active_reference_preserved: true,
        checked_membership: true,
        thumbnail_decode: true,
        cancel_without_publication: true,
        focus_restored: true,
        explicit_apply: true,
        final_localui: true,
      },
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      keepOpen: true,
      result: {
        ok: false,
        phase,
        keySequence,
        trustedInputs,
        pointerEvents,
        releaseEvents,
        error: String(error),
        stack: error?.stack,
        alerts: [...document.querySelectorAll('[role="alert"]')].map((e) => e.textContent),
      },
    });
  }
})();
