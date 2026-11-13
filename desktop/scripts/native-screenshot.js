(async () => {
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const button = (name) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === name);
  const wait = async (predicate, why) => {
    for (let i = 0; i < 300; i++) {
      if (await predicate()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    throw new Error(why);
  };
  const assert = (ok, why) => {
    if (!ok) throw new Error(why);
  };
  try {
    if (
      window.__nativeDesktopPhase === 'cancel-recovery' ||
      window.__nativeDesktopPhase === 'owner-loss'
    ) {
      const ownerLoss = window.__nativeDesktopPhase === 'owner-loss';
      await wait(() => button('Pump screen'), 'device navigation');
      button('Pump screen').click();
      const language = async (lang) => {
        const settings = await rpc('get_display_settings');
        settings.interface_language = lang;
        await rpc('set_display_settings', { settings });
        await wait(
          () => document.documentElement.lang === (lang === 'en-US' ? 'en' : 'ru'),
          'capture recovery language',
        );
      };
      const panel = () =>
        document.querySelector(
          'section[aria-label="Image editor"],section[aria-label="Редактор изображения"]',
        );
      const alerts = () =>
        [...panel().querySelectorAll('[role="alert"]')].map((node) => node.textContent).join('\n');
      const before = JSON.stringify(await rpc('list_named_theme_assets')),
        cases = [];
      let sequence = 0;
      for (const lang of ownerLoss ? ['en-US'] : ['en-US', 'ru-RU']) {
        const en = lang === 'en-US';
        await language(lang);
        await wait(
          () =>
            button(en ? 'Import and edit…' : 'Импорт с редактированием…') &&
            !button(en ? 'Import and edit…' : 'Импорт с редактированием…').disabled,
          'open editor',
        );
        button(en ? 'Import and edit…' : 'Импорт с редактированием…').click();
        const canvas = document.createElement('canvas');
        canvas.width = 96;
        canvas.height = 64;
        canvas.getContext('2d').fillRect(0, 0, 96, 64);
        const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
        const files = new DataTransfer();
        files.items.add(new File([blob], 'kept.png', { type: 'image/png' }));
        await wait(() => panel()?.querySelector('input[type=file]'), 'image input');
        const input = panel().querySelector('input[type=file]');
        input.files = files.files;
        input.dispatchEvent(new Event('change', { bubbles: true }));
        await wait(() => panel().textContent.includes('kept.png · 96 × 64 px'), 'retained source');
        button(en ? 'Take screenshot…' : 'Снять экран…').click();
        await invoke('native_media_step', { sequence: ++sequence, action: 'recovery_ready' });
        button(en ? 'Cancel screenshot' : 'Отменить снимок').click();
        await wait(() => alerts().includes('fixture Close rejected'), 'unconfirmed cancel');
        const id = await invoke('pending_screenshot_cleanup');
        assert(typeof id === 'string' && id.length === 32, 'opaque cleanup recovery id');
        assert(button(en ? 'Take screenshot…' : 'Снять экран…').disabled, 'no overlapping capture');
        assert(
          !panel().textContent.includes(en ? 'Screenshot cancelled.' : 'Снимок экрана отменён.'),
          'no false cancellation notice',
        );
        await language(en ? 'ru-RU' : 'en-US');
        await wait(
          () =>
            alerts().includes(
              en
                ? 'Отмена снимка экрана не подтверждена'
                : 'Screenshot cancellation was not confirmed',
            ),
          'reactive cancellation context',
        );
        button(en ? 'Закрыть редактор изображения' : 'Close image editor').click();
        await new Promise((resolve) => setTimeout(resolve, 30));
        await wait(
          () =>
            button(en ? 'Закрыть редактор изображения' : 'Close image editor') &&
            !button(en ? 'Закрыть редактор изображения' : 'Close image editor').disabled,
          'joined failed editor Close',
        );
        assert(
          !!panel() && alerts().includes('fixture Close rejected'),
          'failed editor Close retains recovery controls',
        );
        await invoke('native_media_step', { sequence: ++sequence, action: 'recovery_allow' });
        button(en ? 'Отменить снимок' : 'Cancel screenshot').click();
        await wait(
          () => !button(en ? 'Снять экран…' : 'Take screenshot…').disabled,
          'explicit cancellation acknowledged',
        );
        assert((await invoke('pending_screenshot_cleanup')) === null, 'cleanup fence released');
        assert(
          panel().textContent.includes('kept.png · 96 × 64 px'),
          'source survives cancellation',
        );
        if (ownerLoss) {
          await wait(
            () =>
              panel().textContent.includes('Портал отключился. Предыдущий запрос снимка завершён.'),
            'owner loss notice instead of cancelled',
          );
          assert(
            !panel().textContent.includes('Снимок экрана отменён.'),
            'owner loss is not a Close acknowledgement',
          );
          button('Снять экран…').click();
          await invoke('native_media_step', {
            sequence: ++sequence,
            action: 'owner_loss_new_ready',
          });
          button('Отменить снимок').click();
          await wait(
            () => !button('Снять экран…').disabled,
            'new capture cancelled on replacement',
          );
          assert(
            panel().textContent.includes('Снимок экрана отменён.'),
            'new capture has its own closed receipt',
          );
        }
        cases.push({ language: lang, id, ownerLoss });
        button(en ? 'Закрыть редактор изображения' : 'Close image editor').click();
        await wait(() => !panel(), 'confirmed editor Close');
      }
      assert(
        JSON.stringify(await rpc('list_named_theme_assets')) === before,
        'cancellation does not publish images',
      );
      await invoke('native_smoke_result', {
        result: { ok: true, recovery: true, cases },
        keepOpen: true,
      });
      return;
    }
    await wait(() => button('Экран помпы'), 'device navigation');
    button('Экран помпы').click();
    await wait(
      () => button('Импорт с редактированием…') && !button('Импорт с редактированием…').disabled,
      'editor available',
    );
    const dev = (await rpc('list_devices'))[0].id;
    const before = (await rpc('list_named_theme_assets'))
      .map((a) => a.key)
      .sort()
      .join(',');
    const lcd = (await rpc('get_lcd_state', { dev })).source;
    button('Импорт с редактированием…').click();
    await wait(() => button('Снять экран…'), 'capture action');
    button('Снять экран…').click();
    await wait(
      () => document.body.textContent.includes('Снимок экрана.png · 64 × 32 px'),
      'raw native screenshot inspected',
    );
    assert(
      (await rpc('list_named_theme_assets'))
        .map((a) => a.key)
        .sort()
        .join(',') === before,
      'capture did not publish',
    );
    button('Подготовить preview изображения').click();
    await wait(() => {
      const img = document.querySelector('img[alt="Результат редактирования 480 на 480"]');
      return img?.complete && img.naturalWidth === 480;
    }, 'native screenshot preview');
    const canvas = document.createElement('canvas');
    canvas.width = 480;
    canvas.height = 480;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(document.querySelector('img[alt="Результат редактирования 480 на 480"]'), 0, 0);
    const left = Array.from(ctx.getImageData(80, 240, 1, 1).data),
      right = Array.from(ctx.getImageData(400, 240, 1, 1).data);
    assert(
      left[0] > 245 && left[2] < 10 && right[2] > 245 && right[0] < 10,
      'screenshot pixels preserved by native IPC and crop',
    );
    button('Сохранить изображение в галерею').click();
    let saved;
    await wait(async () => {
      saved = (await rpc('list_named_theme_assets')).find(
        (a) => !a.is_default && a.name === 'Снимок экрана.png',
      );
      return !!saved;
    }, 'screenshot published by Save');
    assert((await rpc('get_lcd_state', { dev })).source === lcd, 'screenshot did not Apply');
    await wait(
      () => button('Снять экран…') && !button('Снять экран…').disabled,
      'capture ready after Save',
    );
    button('Снять экран…').click();
    await wait(
      () => document.body.textContent.includes('Снимок экрана отменён.'),
      'portal cancellation',
    );
    assert(
      document.body.textContent.includes('Снимок экрана.png · 64 × 32 px'),
      'cancel retains source',
    );
    button('Снять экран…').click();
    await invoke('native_media_step', { sequence: 1, action: 'screenshot_cancel_ready' });
    button('Закрыть редактор изображения').click();
    await wait(
      () => !document.querySelector('[aria-label="Редактор изображения"]'),
      'editor close while capturing',
    );
    await wait(
      () => document.activeElement === button('Импорт с редактированием…'),
      'focus after cancelling screenshot',
    );
    button('Импорт с редактированием…').click();
    await wait(() => button('Снять экран…'), 'reopen screenshot editor');
    button('Снять экран…').click();
    await invoke('native_media_step', { sequence: 2, action: 'screenshot_close_ready' });
    await invoke('native_smoke_result', {
      result: {
        ok: true,
        raw_bytes: true,
        preview_pixels: { left, right },
        saved_key: saved.key,
        portal_cancel: true,
        editor_cancel: true,
        close_pending: true,
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
