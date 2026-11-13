(async () => {
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  // Fixed expected labels keep the native workflow independent of runtime translations.
  const englishLabels = {
    'button[aria-label="Выбор области видео"]': 'button[aria-label="Video region selection"]',
    'button[aria-label="Выбор области захвата на кадре"]':
      'button[aria-label="Capture region selection on frame"]',
    'canvas[aria-label="Предпросмотр области захвата"]':
      'canvas[aria-label="Capture region preview"]',
    'video[aria-label="Готовый ролик"]': 'video[aria-label="Prepared video"]',
    'video[aria-label="Исходное видео"]': 'video[aria-label="Source video"]',
    Библиотека: 'Library',
    'Библиотека роликов': 'Video library',
    'Вернуться к редактированию записи': 'Return to recording edits',
    'Выбор области видео': 'Video region selection',
    'Выбор области захвата на кадре': 'Capture region selection on frame',
    'Выбрать экран или окно': 'Select screen or window',
    'Высота захвата, %': 'Capture height, %',
    'Высота области, %': 'Region height, %',
    'Завершить и просмотреть запись': 'Finish and review recording',
    'Закрыть запись экрана': 'Close screen recording',
    'Записать экран…': 'Record screen…',
    'Захват X, %': 'Capture X, %',
    'Захват Y, %': 'Capture Y, %',
    'Начать запись экрана': 'Start screen recording',
    'Область X, %': 'Region X, %',
    'Область Y, %': 'Region Y, %',
    'Обрезать готовую запись': 'Trim prepared recording',
    'Подготовить запись для LCD': 'Prepare recording for LCD',
    'Подготовить запись целиком': 'Prepare entire recording',
    'Применить область захвата': 'Apply capture region',
    'Приостановить запись': 'Pause recording',
    'Продолжить запись': 'Resume recording',
    'Редактировать запись': 'Edit recording',
    'Сохранить запись в библиотеку': 'Save recording to library',
    'Ширина захвата, %': 'Capture width, %',
    'Ширина области, %': 'Region width, %',
    'Экран помпы': 'Pump screen',
  };
  let language = 'ru-RU';
  const text = (value) => (language === 'en-US' ? englishLabels[value] : value);
  const button = (name) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === name);
  const wait = async (predicate, why) => {
    for (let i = 0; i < 400; i++) {
      if (await predicate()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    throw new Error(why);
  };
  const assert = (ok, why) => {
    if (!ok) throw new Error(why);
  };
  const click = async (name) => {
    await wait(() => button(name) && !button(name).disabled, name);
    button(name).click();
  };
  const fill = async (label, value) => {
    const input = () =>
      [...document.querySelectorAll('label')]
        .find((e) => e.textContent.trim() === label)
        ?.querySelector('input');
    await wait(() => input() && !input().disabled, label);
    input().value = String(value);
    input().dispatchEvent(new Event('input', { bubbles: true }));
  };
  let pointerSequence = 0;
  const trustedRegionEvents = [];
  async function trustedRegionSelection(kind = 'capture') {
    const capture = kind === 'capture';
    const selector = document.querySelector(
      `button[aria-label="${capture ? text('Выбор области захвата на кадре') : text('Выбор области видео')}"]`,
    );
    const fields = capture
      ? [
          text('Захват X, %'),
          text('Захват Y, %'),
          text('Ширина захвата, %'),
          text('Высота захвата, %'),
        ]
      : [
          text('Область X, %'),
          text('Область Y, %'),
          text('Ширина области, %'),
          text('Высота области, %'),
        ];
    const values = () =>
      fields.map(
        (name) =>
          [...document.querySelectorAll('label')]
            .find((e) => e.textContent.trim() === name)
            .querySelector('input').valueAsNumber,
      );
    for (const mode of ['outside', 'capture-loss', 'fresh']) {
      selector.scrollIntoView({ block: 'center' });
      selector.focus();
      let released = false,
        afterUp;
      const trace = (event) => {
        if (!event.isTrusted || !selector.contains(event.target)) return;
        trustedRegionEvents.push({ kind, mode, type: event.type, buttons: event.buttons });
        if (
          mode === 'capture-loss' &&
          event.type === 'pointermove' &&
          event.buttons & 1 &&
          selector.hasPointerCapture(event.pointerId)
        ) {
          selector.releasePointerCapture(event.pointerId);
          released = true;
        }
        if (event.type === 'pointerup')
          queueMicrotask(() => {
            afterUp = values();
          });
      };
      for (const type of ['pointerdown', 'pointermove', 'pointerup'])
        document.addEventListener(type, trace);
      let keyup, timer;
      const done = new Promise((resolve) => {
        keyup = (event) => {
          if (event.isTrusted && event.key === 'Home') resolve();
        };
        document.addEventListener('keyup', keyup, true);
      });
      try {
        await invoke('native_keyboard_step', {
          sequence: ++pointerSequence,
          key: 'Home',
          probe: capture ? 'capture_region_pointer' : 'video_region_pointer',
        });
        await Promise.race([
          done,
          new Promise(
            (_, reject) =>
              (timer = setTimeout(() => reject(new Error('native region input deadline')), 10000)),
          ),
        ]);
        await new Promise((resolve) => setTimeout(resolve, 80));
        const actual = values(),
          expected =
            mode === 'outside' || (!capture && mode === 'capture-loss')
              ? [20, 20, 80, 80]
              : [20, 20, 40, 40];
        assert(
          actual.every((value, i) => Math.abs(value - expected[i]) < 1),
          'trusted ' + mode + ' region ' + JSON.stringify(actual),
        );
        assert(
          afterUp && actual.every((value, i) => value === afterUp[i]),
          'released return must not change region',
        );
        assert(
          mode !== 'capture-loss' || released,
          'capture explicitly lost during held native gesture',
        );
        assert(
          trustedRegionEvents.some(
            (e) => e.kind === kind && e.mode === mode && e.type === 'pointerdown',
          ) &&
            trustedRegionEvents.some(
              (e) => e.kind === kind && e.mode === mode && e.type === 'pointerup',
            ),
          'trusted down/up ' + mode,
        );
        if (capture)
          assert(
            (await invoke('screen_record_status')).region.x === 0.9,
            'local selection must not apply capture region',
          );
      } finally {
        clearTimeout(timer);
        document.removeEventListener('keyup', keyup, true);
        for (const type of ['pointerdown', 'pointermove', 'pointerup'])
          document.removeEventListener(type, trace);
      }
    }
  }
  function assertVideoDraft() {
    const expected = [20, 20, 40, 40];
    const actual = [
      text('Область X, %'),
      text('Область Y, %'),
      text('Ширина области, %'),
      text('Высота области, %'),
    ].map(
      (name) =>
        [...document.querySelectorAll('label')]
          .find((e) => e.textContent.trim() === name)
          ?.querySelector('input')?.valueAsNumber,
    );
    assert(
      actual.every((value, i) => Math.abs(value - expected[i]) < 1),
      'video crop retained across review recovery ' + JSON.stringify(actual),
    );
  }
  try {
    language = (await rpc('get_display_settings')).interface_language;
    assert(['en-US', 'ru-RU'].includes(language), 'supported native language');
    await wait(async () => (await rpc('list_devices')).length > 0, 'mock device');
    const dev = (await rpc('list_devices'))[0].id;
    await rpc('migrate_screen_collections', { dev });
    await click(text('Экран помпы'));
    await click(text('Библиотека'));
    await wait(
      () =>
        [...document.querySelectorAll('summary')].find(
          (e) => e.textContent === text('Библиотека роликов'),
        ),
      'video library',
    );
    [...document.querySelectorAll('summary')]
      .find((e) => e.textContent === text('Библиотека роликов'))
      .click();
    await click(text('Записать экран…'));
    await click(text('Выбрать экран или окно'));
    await wait(
      async () => (await invoke('screen_record_status'))?.phase === 'setup',
      'initial setup',
    );
    await wait(
      () => document.querySelector(text('button[aria-label="Выбор области захвата на кадре"]')),
      'initial source preview',
    );
    await fill(text('Захват X, %'), 90);
    await fill(text('Захват Y, %'), 20);
    await fill(text('Ширина захвата, %'), 6);
    await fill(text('Высота захвата, %'), 5);
    await click(text('Применить область захвата'));
    await wait(
      async () => (await invoke('screen_record_status')).region?.x === 0.9,
      'initial region readback',
    );
    await new Promise((resolve) => setTimeout(resolve, 500));
    const setupState = await invoke('screen_record_status');
    if (setupState.frames !== 0 || setupState.recordedMillis !== 0)
      throw new Error('Setup recorded frames or time');
    await click(text('Начать запись экрана'));
    await wait(
      async () => (await invoke('screen_record_status'))?.frames >= 30,
      'synthetic recording frames',
    );
    await click(text('Приостановить запись'));
    await wait(async () => (await invoke('screen_record_status'))?.phase === 'paused', 'pause');
    await wait(
      () => document.querySelector(text('button[aria-label="Выбор области захвата на кадре"]')),
      'source preview control',
    );
    await trustedRegionSelection();
    await fill(text('Захват X, %'), 75);
    await fill(text('Захват Y, %'), 20);
    await fill(text('Ширина захвата, %'), 6);
    await fill(text('Высота захвата, %'), 5);
    const selector = document.querySelector(
      text('button[aria-label="Выбор области захвата на кадре"]'),
    );
    selector.focus();
    selector.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await wait(
      () =>
        [...document.querySelectorAll('label')]
          .find((e) => e.textContent.trim() === text('Захват X, %'))
          .querySelector('input').valueAsNumber === 76,
      'keyboard region move',
    );
    selector.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    await wait(() => {
      const canvas = document.querySelector(
        text('canvas[aria-label="Предпросмотр области захвата"]'),
      );
      if (!canvas) return false;
      const pixel = canvas
        .getContext('2d')
        .getImageData(canvas.width / 2, canvas.height / 2, 1, 1).data;
      return pixel[0] > 200 && pixel[1] < 35 && pixel[2] < 35;
    }, 'red region visual preview');
    await click(text('Применить область захвата'));
    await wait(
      async () => (await invoke('screen_record_status')).region?.x === 0.75,
      'red region readback',
    );
    const pausedFrames = (await invoke('screen_record_status')).frames;
    await click(text('Продолжить запись'));
    await wait(
      async () => (await invoke('screen_record_status')).frames >= pausedFrames + 6,
      'frames after resume',
    );
    await click(text('Приостановить запись'));
    await wait(
      async () => (await invoke('screen_record_status')).phase === 'paused',
      'second pause',
    );
    const snapshotStatus = await invoke('screen_record_status');
    const snapshotBytes = await invoke('screen_record_snapshot', { id: snapshotStatus.id });
    const snapshotUrl = URL.createObjectURL(
      new Blob([new Uint8Array(snapshotBytes)], { type: 'image/jpeg' }),
    );
    try {
      const picture = new Image();
      picture.src = snapshotUrl;
      await picture.decode();
      assert(
        picture.naturalWidth === 320 && picture.naturalHeight === 240,
        'full-source snapshot dimensions',
      );
      const canvas = document.createElement('canvas');
      canvas.width = 320;
      canvas.height = 240;
      const context = canvas.getContext('2d');
      context.drawImage(picture, 0, 0);
      const blue = context.getImageData(288, 48, 1, 1).data;
      assert(
        blue[2] > 200 && blue[0] < 35 && blue[1] < 35,
        'snapshot retains blue area outside active red crop',
      );
      assert(
        (await invoke('screen_record_status')).frames === snapshotStatus.frames,
        'snapshot does not resume capture',
      );
    } finally {
      URL.revokeObjectURL(snapshotUrl);
    }
    await fill(text('Захват X, %'), 90);
    await click(text('Применить область захвата'));
    await wait(
      async () => (await invoke('screen_record_status')).region?.x === 0.9,
      'blue region readback',
    );
    const secondPause = (await invoke('screen_record_status')).frames;
    await click(text('Продолжить запись'));
    await wait(
      async () => (await invoke('screen_record_status')).frames >= secondPause + 6,
      'frames after second region',
    );
    await click(text('Завершить и просмотреть запись'));
    await click(text('Редактировать запись'));
    await wait(
      () => document.querySelector(text('video[aria-label="Исходное видео"]'))?.readyState >= 2,
      'native source decode',
    );
    const source = document.querySelector(text('video[aria-label="Исходное видео"]'));
    const dimensions = [source.videoWidth, source.videoHeight];
    source.currentTime = 0.5;
    await wait(() => !source.seeking && Math.abs(source.currentTime - 0.5) < 0.05, 'source seek');
    const before = (await rpc('list_prepared_clips')).map((c) => c.key);
    const trim = [...document.querySelectorAll('label')]
      .find((label) => label.textContent.trim() === text('Обрезать готовую запись'))
      .querySelector('input');
    if (!trim.checked) trim.click();
    await wait(
      () => document.querySelector(text('button[aria-label="Выбор области видео"]')),
      'video crop selector',
    );
    await trustedRegionSelection('video');
    assert(
      (await invoke('screen_record_status')).phase === 'ready',
      'video crop does not start preparation',
    );
    assert(
      (await rpc('list_prepared_clips')).every((c) => before.includes(c.key)),
      'video crop does not publish',
    );
    // One-shot transport failures exercise production lease cleanup and UI recovery.
    const originalFetch = window.fetch;
    try {
      for (const command of ['screen_record_admit', 'screen_record_prepare']) {
        let rejected = false;
        window.fetch = (input, options) => {
          const url = new URL(input instanceof Request ? input.url : String(input), location.href);
          if (url.pathname.endsWith('/' + command) && !rejected) {
            rejected = true;
            return Promise.resolve(
              new Response(JSON.stringify('injected ' + command + ' rejection'), {
                headers: { 'Content-Type': 'application/json', 'Tauri-Response': 'error' },
              }),
            );
          }
          return originalFetch(input, options);
        };
        await click(text('Подготовить запись для LCD'));
        await wait(
          () =>
            [...document.querySelectorAll('[role="alert"]')].some((e) =>
              e.textContent.includes('injected ' + command),
            ),
          command + ' surfaced',
        );
        await click(text('Вернуться к редактированию записи'));
        await wait(
          () =>
            button(text('Подготовить запись для LCD')) &&
            !button(text('Подготовить запись для LCD')).disabled,
          'edit restored',
        );
        assert(
          (await invoke('screen_record_status')).phase === 'ready',
          'failed review retains capture',
        );
        assertVideoDraft();
        assert(
          (await rpc('list_prepared_clips')).every((c) => before.includes(c.key)),
          'failed review never published',
        );
        if (command === 'screen_record_prepare')
          assert(
            (await invoke('media_status')).phase === 'cancelled',
            'admitted review cancelled before editing',
          );
      }
    } finally {
      window.fetch = originalFetch;
    }
    await click(text('Подготовить запись для LCD'));
    await wait(
      () => document.querySelector(text('video[aria-label="Готовый ролик"]'))?.readyState >= 2,
      'native prepared decode',
    );
    assert(
      (await rpc('list_prepared_clips')).every((c) => before.includes(c.key)),
      'no publication before Save',
    );
    const firstReview = (await invoke('media_status')).id;
    await click(text('Вернуться к редактированию записи'));
    await wait(
      () => document.querySelector(text('video[aria-label="Исходное видео"]'))?.readyState >= 2,
      'source retained after review cancellation',
    );
    assert((await invoke('screen_record_status')).phase === 'ready', 'capture retained');
    assertVideoDraft();
    await click(text('Подготовить запись для LCD'));
    await wait(
      () => document.querySelector(text('video[aria-label="Готовый ролик"]'))?.readyState >= 2,
      'second review decode',
    );
    assert((await invoke('media_status')).id !== firstReview, 'new review ID');
    assert(
      (await rpc('list_prepared_clips')).every((c) => before.includes(c.key)),
      'cancelled review never published',
    );
    await click(text('Сохранить запись в библиотеку'));
    await wait(
      async () => (await invoke('media_status'))?.phase === 'complete',
      'native publication',
    );
    const saved = (await invoke('media_status')).key;
    await wait(async () => {
      const s = await rpc('get_screen_collections', { dev });
      return (
        !!s.collections &&
        s.collections.backgrounds.some((a) => a.kind === 'video' && a.key === saved)
      );
    }, 'collection membership');
    await click(text('Закрыть запись экрана'));
    await wait(
      async () => (await invoke('screen_record_status'))?.phase === 'cancelled',
      'capture lease cleanup',
    );
    await click(text('Записать экран…'));
    await click(text('Выбрать экран или окно'));
    await click(text('Начать запись экрана'));
    await wait(
      async () => (await invoke('screen_record_status'))?.frames >= 12,
      'whole import recording',
    );
    const beforeWhole = (await rpc('list_prepared_clips')).map((c) => c.key);
    await click(text('Завершить и просмотреть запись'));
    await click(text('Подготовить запись целиком'));
    await wait(
      () => document.querySelector(text('video[aria-label="Готовый ролик"]'))?.readyState >= 2,
      'whole recording prepared decode',
    );
    assert(
      (await rpc('list_prepared_clips')).every((c) => beforeWhole.includes(c.key)),
      'whole import awaits explicit Save',
    );
    await click(text('Закрыть запись экрана'));
    await wait(
      async () => (await invoke('screen_record_status'))?.phase === 'cancelled',
      'whole import cleanup',
    );
    await click(text('Записать экран…'));
    await click(text('Выбрать экран или окно'));
    await click(text('Начать запись экрана'));
    await wait(
      async () => (await invoke('screen_record_status'))?.phase === 'recording',
      'third capture for window close',
    );
    await invoke('native_smoke_result', {
      result: {
        ok: true,
        language,
        trusted_region_steps: pointerSequence,
        trusted_video_crop: true,
        video_crop_recovery: true,
        trusted_region_events: trustedRegionEvents,
        source_dimensions: dimensions,
        source_seek: true,
        prepared_decode: true,
        explicit_save: true,
        review_reedit: true,
        admission_failure_recovery: true,
        prepare_failure_recovery: true,
        operation_choice: true,
        capture_regions: true,
        source_snapshot: true,
        visual_region_preview: true,
        initial_region_setup: true,
        resumed_frames: true,
        collection_membership: true,
        saved_key: saved,
        close_while_recording: true,
      },
      keepOpen: true,
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      result: {
        ok: false,
        error: String(error),
        stack: error?.stack,
        alerts: [...document.querySelectorAll('[role="alert"]')].map((e) => e.textContent),
      },
    });
  }
})();
