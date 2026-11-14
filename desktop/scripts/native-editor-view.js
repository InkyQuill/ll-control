// Native-smoke-only inspection of a private MockBackend document. No device Apply.
(async () => {
  const invoke = window.__TAURI_INTERNALS__.invoke;
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const wait = async (check) => {
    for (let i = 0; i < 600; i++) {
      if (check()) return;
      await delay(50);
    }
    throw new Error('editor view timeout');
  };
  const en = window.__nativeDesktopLanguage === 'en-US';
  const click = async (english, russian) => {
    const name = en ? english : russian;
    await wait(() =>
      [...document.querySelectorAll('button')].some(
        (n) => (n.getAttribute('aria-label') || n.textContent.trim()) === name && !n.disabled,
      ),
    );
    [...document.querySelectorAll('button')]
      .find((n) => (n.getAttribute('aria-label') || n.textContent.trim()) === name && !n.disabled)
      .click();
  };
  try {
    await click('Pump screen', 'Экран помпы');
    await click('Composition', 'Композиция');
    await click('Load or stop', 'Загрузить или остановить');
    await click('Load active theme', 'Загрузить активную тему');
    await wait(() => document.querySelectorAll('.layer-row[data-layer-index]').length >= 7);
    document.querySelector('.layer-row[data-layer-index="1"]').click();
    document.querySelectorAll('[popover]:popover-open').forEach((n) => n.hidePopover());
    await wait(
      () => document.querySelector('.canvas img') && !document.querySelector('.canvas .pending'),
    );
    const fonts = await invoke('system_fonts');
    if (!fonts.length) throw new Error('Installed font catalogue is empty');
    const font =
      fonts.find((font) => font.name === 'DejaVu Sans · Book') ??
      fonts.find((font) => font.name.startsWith('DejaVu Sans ·')) ??
      fonts[0];
    const fontSelect = document.querySelector('.inspector select[id]');
    await wait(() =>
      [...fontSelect.options].some((option) => option.value === `system:${font.id}`),
    );
    fontSelect.value = `system:${font.id}`;
    fontSelect.dispatchEvent(new Event('change', { bubbles: true }));
    await wait(() => fontSelect.value.startsWith('asset:') && !fontSelect.disabled);
    await wait(
      () => document.querySelector('.canvas img') && !document.querySelector('.canvas .pending'),
    );

    document.activeElement?.blur();
    window.scrollTo(0, 0);
    const frames = [];
    const started = performance.now();
    const observer = new MutationObserver((changes) => {
      if (changes.some((c) => c.attributeName === 'src')) frames.push(performance.now());
    });
    observer.observe(document.querySelector('.canvas'), {
      subtree: true,
      attributes: true,
      attributeFilter: ['src'],
    });
    await delay(10000);
    observer.disconnect();
    const previewFps = frames.length / ((performance.now() - started) / 1000);
    const samples = { property: [], keyboardMove: [], drag: [] };
    const afterPaint = () =>
      new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const pixel = () => document.querySelector('.canvas img');
    for (let i = 0; i < 30; i++) {
      const input = [...document.querySelectorAll('.inspector label')]
        .find((label) => label.textContent.trim() === (en ? 'Label' : 'Метка'))
        ?.querySelector('input');
      if (!input || !pixel()) throw new Error('missing sensor label or layer pixels');
      const previous = pixel().src;
      let time = performance.now();
      input.value = i % 2 ? 'CPU1' : 'Core one';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      while (pixel()?.src === previous || !pixel()) {
        if (performance.now() - time > 5000) throw new Error('label pixel timeout');
        await new Promise(requestAnimationFrame);
      }
      await pixel().decode();
      await new Promise(requestAnimationFrame);
      samples.property.push(performance.now() - time);
      await wait(
        () =>
          document.querySelector('.canvas .region.selected')?.getAttribute('aria-disabled') ===
          'false',
      );
      const region = document.querySelector('.canvas .region.selected');
      const rect = region.getBoundingClientRect();
      const canvas = document.querySelector('.canvas');
      const pointer = (kind, dx) =>
        new PointerEvent(kind, {
          bubbles: true,
          button: 0,
          buttons: kind === 'pointerup' ? 0 : 1,
          pointerId: 1,
          pointerType: 'mouse',
          clientX: rect.x + rect.width / 2 + dx,
          clientY: rect.y + rect.height / 2,
        });
      region.dispatchEvent(pointer('pointerdown', 0));
      await afterPaint();
      time = performance.now();
      canvas.dispatchEvent(pointer('pointermove', 8));
      while (region.getBoundingClientRect().left === rect.left) {
        if (performance.now() - time > 5000) throw new Error('drag feedback timeout');
        await new Promise(requestAnimationFrame);
      }
      await new Promise(requestAnimationFrame);
      samples.drag.push(performance.now() - time);
      canvas.dispatchEvent(pointer('pointercancel', 0));
      await afterPaint();
      const left = region.getBoundingClientRect().left;
      time = performance.now();
      region.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: i % 2 ? 'ArrowLeft' : 'ArrowRight',
          bubbles: true,
          ctrlKey: true,
        }),
      );
      while (
        document.querySelector('.canvas .region.selected')?.getBoundingClientRect().left === left
      ) {
        if (performance.now() - time > 5000) throw new Error('movement timeout');
        await new Promise(requestAnimationFrame);
      }
      await afterPaint();
      samples.keyboardMove.push(performance.now() - time);
    }
    const apply = [...document.querySelectorAll('button')].find(
      (n) => n.textContent.trim() === (en ? 'Apply to LCD' : 'Применить к LCD'),
    );
    const rect = (n) => {
      const r = n.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom };
    };
    const result = {
      ok: true,
      systemFont: { catalogue: fonts.length, name: font.name, asset: fontSelect.value },
      viewport: { width: innerWidth, height: innerHeight, scale: devicePixelRatio },
      frames: frames.length,
      fps: previewFps,
      samples,
      layers: document.querySelectorAll('.layer-row[data-layer-index]').length,
      apply: rect(apply),
      inspector: rect(document.querySelector('.inspector')),
      canvas: rect(document.querySelector('.canvas')),
      horizontalOverflow: document.documentElement.scrollWidth > innerWidth,
    };
    await invoke('native_smoke_result', { keepOpen: true, result });
    if (window.__nativeEditorSoak) {
      let edits = 0;
      while (true) {
        await delay(2000);
        const input = [...document.querySelectorAll('.inspector label')]
          .find((label) => label.textContent.trim() === (en ? 'Label' : 'Метка'))
          ?.querySelector('input');
        if (!input) throw new Error('sensor label disappeared during prolonged editing');
        input.value = 'CPU ' + (++edits % 100);
        input.dispatchEvent(new Event('input', { bubbles: true }));
        await invoke('native_smoke_result', { keepOpen: true, result: { ...result, edits } });
      }
    }
    await delay(3000);
    await click('Layers', 'Слои');
    await delay(300);
    await invoke('native_smoke_result', { keepOpen: true, result: { ...result, pane: 'layers' } });
  } catch (error) {
    await invoke('native_smoke_result', { result: { ok: false, error: String(error) } });
  }
})();
