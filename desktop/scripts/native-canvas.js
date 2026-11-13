// Runs only in the native-smoke build on a private MockBackend socket.
(async () => {
  if (window.__nativeCanvasStarted) return;
  window.__nativeCanvasStarted = true;
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const assert = (value, message) => {
    if (!value) throw new Error(message);
  };
  const wait = async (check, why) => {
    for (let i = 0; i < 400; i++) {
      if (await check()) return;
      await delay(50);
    }
    throw new Error('timeout: ' + why);
  };
  let phase = 'setup',
    sequence = 0,
    trustedKeys = 0,
    trustedPointers = 0;
  const events = [];
  for (const kind of ['pointerdown', 'pointerup', 'pointercancel', 'lostpointercapture', 'keydown'])
    document.addEventListener(
      kind,
      (event) => {
        if (event.isTrusted) events.push({ kind, key: event.key, buttons: event.buttons, phase });
      },
      true,
    );
  const visible = (node) => node?.getClientRects().length && !node.closest('[hidden]');
  const text = (en, ru) => (window.__nativeDesktopLanguage === 'en-US' ? en : ru);
  const button = (name) =>
    [...document.querySelectorAll('button')].find(
      (node) =>
        visible(node) && (node.getAttribute('aria-label') || node.textContent.trim()) === name,
    );
  const click = async (name) => {
    await wait(() => button(name) && !button(name).disabled, name);
    button(name).click();
    await delay(30);
  };
  const control = (name, root = document) =>
    [...root.querySelectorAll('label')]
      .find(
        (node) =>
          [...node.childNodes]
            .filter((child) => child.nodeType === Node.TEXT_NODE)
            .map((child) => child.textContent)
            .join('')
            .trim() === name,
      )
      ?.querySelector('input,select,textarea');
  const set = (name, value, root = document) => {
    const node = control(name, root);
    assert(node, 'control ' + name);
    node.value = String(value);
    node.dispatchEvent(
      new Event(node.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }),
    );
  };
  const layers = () => [...document.querySelectorAll('.layer-row[data-layer-index]')];
  const inspector = () => document.querySelector('.inspector');
  const select = async (index) => {
    document.querySelector(`.layer-row[data-layer-index="${index}"]`).click();
    await delay(30);
  };
  const open = async (en, ru) => {
    const trigger = button(text(en, ru));
    if (trigger) {
      const panel = document.getElementById(trigger.getAttribute('popovertarget'));
      if (panel && !panel.matches(':popover-open')) {
        trigger.click();
        await delay(30);
      }
    }
  };
  const ready = async () => {
    await wait(
      () => document.querySelector('.canvas img') && !document.querySelector('.canvas .pending'),
      'measured preview ready',
    );
    await delay(60);
  };
  const field = (index, name) => control(name, inspector());
  const region = (index) =>
    button(
      text(
        `Select layer ${index + 1}: ${['Image', 'Text', 'Clock', 'Sensor'][index]}`,
        `Выбрать слой ${index + 1}: ${['Изображение', 'Текст', 'Часы', 'Датчик'][index]}`,
      ),
    );
  document.addEventListener(
    'keydown',
    (event) => {
      if (event.isTrusted) trustedKeys++;
    },
    true,
  );
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (event.isTrusted && event.target.closest('.canvas')) trustedPointers++;
    },
    true,
  );
  async function key(value, probe = null) {
    const expected = {
      Home: 'Home',
      Return: 'Enter',
      Right: 'ArrowRight',
      'ctrl+Right': 'ArrowRight',
      'shift+Right': 'ArrowRight',
    }[value];
    let listener, timer;
    const received = new Promise((resolve, reject) => {
      listener = (event) => {
        if (event.isTrusted && event.key === expected) resolve();
      };
      document.addEventListener('keyup', listener, true);
      timer = setTimeout(() => reject(new Error('native key ' + value)), 10000);
    });
    try {
      await invoke('native_keyboard_step', { sequence: ++sequence, key: value, probe });
      await received;
    } finally {
      clearTimeout(timer);
      document.removeEventListener('keyup', listener, true);
    }
    await delay(80);
  }
  try {
    await wait(async () => (await rpc('list_devices')).length === 1, 'mock device');
    const dev = (await rpc('list_devices'))[0].id;
    await click(text('Pump screen', 'Экран помпы'));
    await click(text('Composition', 'Композиция'));
    const restored = await rpc('get_lcd_theme', { dev });
    if (restored?.name === 'Native canvas acceptance') {
      await open('Load or stop', 'Загрузить или остановить');
      await click(text('Load active theme', 'Загрузить активную тему'));
      await wait(() => layers().length === 7, 'reopened composition');
      await select(1);
      assert(field(1, text('Text', 'Текст')).value === 'Native canvas text', 'reopened saved text');
      assert(Number(field(1, 'X').value) === restored.layers[1].style.x, 'reopened saved position');
      await ready();
      await invoke('native_smoke_result', {
        keepOpen: true,
        result: { ok: true, recovery: true, theme: restored },
      });
      return;
    }
    phase = 'create';
    set(text('Name', 'Название'), 'Native canvas acceptance');
    await open('Add layer', 'Добавить слой');
    await wait(
      () =>
        control(text('Bind sensor', 'Привязать датчик'))?.querySelector(
          'option[value="amdgpu:2f61:utilization"]',
        ),
      'GPU sensor',
    );
    set(text('Bind sensor', 'Привязать датчик'), 'amdgpu:2f61:utilization');
    await click(text('Add', 'Добавить'));
    for (let index = 0; index < 3; index++) {
      await open('Add layer', 'Добавить слой');
      await click(text('+ Text', '+ Текст'));
    }
    await click(text('Library', 'Библиотека'));
    const gallery = button(text('Image library', 'Библиотека изображений'));
    assert(gallery, 'image library');
    gallery.click();
    const addImage = (node) =>
      visible(node) &&
      !node.disabled &&
      (window.__nativeDesktopLanguage === 'en-US'
        ? /^Add .+ to theme$/.test(node.textContent.trim())
        : node.textContent.trim().startsWith('Добавить в тему '));
    await wait(() => [...document.querySelectorAll('button')].some(addImage), 'builtin image');
    [...document.querySelectorAll('button')].find(addImage).click();
    await click(text('Composition', 'Композиция'));
    await wait(() => layers().length === 7, 'seven layers');
    await select(0);
    for (const [name, value] of [
      ['x', 20],
      ['y', 20],
      ['width', 100],
      ['height', 80],
    ])
      set(name, value, inspector());
    set(text('Scale', 'Масштаб'), 'stretch', inspector());
    await select(1);
    set(text('Text', 'Текст'), 'Native canvas text', inspector());
    for (const [index, x, y, size] of [
      [1, 150, 80, 28],
      [2, 80, 320, 22],
      [3, 180, 220, 26],
    ]) {
      await select(index);
      for (const [name, value] of [
        ['X', x],
        ['Y', y],
        [text('Size', 'Размер'), size],
      ])
        set(name, value, inspector());
      set(text('Alignment', 'Выравнивание'), 'left', inspector());
    }
    await select(2);
    set(text('Clock content', 'Содержимое часов'), 'date', inspector());
    await select(3);
    set(text('Label', 'Подпись'), 'Sensor', inspector());
    await ready();
    for (const [index, label, y] of [
      [4, 'CPU', 360],
      [5, 'GPU', 390],
      [6, 'WATER', 420],
    ]) {
      await select(index);
      set(text('Text', 'Текст'), label, inspector());
      set('X', 24, inspector());
      set('Y', y, inspector());
      set(text('Size', 'Размер'), 16, inspector());
    }
    phase = 'pointer-move';
    await select(1);
    await ready();
    const beforeX = Number(field(1, 'X').value),
      beforeY = Number(field(1, 'Y').value);
    region(1).scrollIntoView({ block: 'center' });
    region(1).focus();
    const scale = 480 / document.querySelector('.canvas').getBoundingClientRect().width;
    await key('Home', 'canvas_move');
    await ready();
    const movedX = Number(field(1, 'X').value),
      movedY = Number(field(1, 'Y').value);
    assert(
      Math.abs(movedX - beforeX - 24 * scale) <= 2 && Math.abs(movedY - beforeY - 16 * scale) <= 2,
      'native move maps to 480px',
    );
    phase = 'pointer-cancel';
    region(1).scrollIntoView({ block: 'center' });
    region(1).focus();
    await key('Home', 'canvas_cancel');
    await ready();
    assert(
      Number(field(1, 'X').value) === movedX && Number(field(1, 'Y').value) === movedY,
      `Escape discards drag: before ${movedX},${movedY}; after ${field(1, 'X').value},${field(1, 'Y').value}`,
    );
    phase = 'history';
    await click(text('Undo edit', 'Отменить правку'));
    await ready();
    assert(Number(field(1, 'X').value) === beforeX, 'undo move');
    await click(text('Redo edit', 'Повторить правку'));
    await ready();
    assert(Number(field(1, 'X').value) === movedX, 'redo move');
    phase = 'keyboard';
    region(1).scrollIntoView({ block: 'center' });
    region(1).focus();
    await key('ctrl+Right');
    await ready();
    assert(Number(field(1, 'X').value) === movedX + 10, 'trusted keyboard movement');
    const size = Number(field(1, text('Size', 'Размер')).value);
    await key('shift+Right');
    await ready();
    assert(
      Number(field(1, text('Size', 'Размер')).value) === size + 1,
      'trusted keyboard resize retains focus',
    );
    phase = 'image-resize';
    await select(0);
    const handle = button(text('Resize selected layer', 'Изменить размер выбранного слоя'));
    handle.scrollIntoView({ block: 'center' });
    handle.focus();
    await key('Home', 'canvas_resize');
    await ready();
    assert(Number(field(0, 'width').value) > 100, 'trusted image resize');
    phase = 'rebind';
    await select(3);
    const x = field(3, 'X').value,
      y = field(3, 'Y').value,
      label = field(3, text('Label', 'Подпись')).value;
    set(text('Sensor source', 'Источник датчика'), 'fixture:cpu', inspector());
    await ready();
    assert(
      field(3, 'X').value === x &&
        field(3, 'Y').value === y &&
        field(3, text('Label', 'Подпись')).value === label,
      'source replacement preserves authored properties',
    );
    phase = 'save';
    await open('Saved themes', 'Сохранённые темы');
    await open('Saved profiles', 'Сохранённые профили');
    await click(text('Refresh profile list', 'Обновить список профилей'));
    set(text('Profile name', 'Имя профиля'), 'Native canvas profile');
    await click(text('Save draft to slot', 'Сохранить черновик в слот'));
    await wait(async () => (await rpc('list_profiles')).profiles.length === 1, 'saved profile');
    const saved = (await rpc('list_profiles')).profiles[0].content.value;
    assert((await rpc('get_lcd_theme', { dev })) === null, 'Save and edits never Apply');
    await select(1);
    set(text('Text', 'Текст'), 'Discard this change', inspector());
    await click(text('Load profile into editor', 'Загрузить профиль в редактор'));
    await select(1);
    await ready();
    assert(field(1, text('Text', 'Текст')).value === 'Native canvas text', 'profile restores text');
    assert(Number(field(1, 'X').value) === movedX + 10, 'profile restores transforms');
    await key('Home', 'canvas_no_writes');
    phase = 'apply';
    await click(text('Apply to LCD', 'Применить к LCD'));
    await wait(
      async () => (await rpc('get_lcd_theme', { dev }))?.name === 'Native canvas acceptance',
      'explicit Apply',
    );
    const applied = await rpc('get_lcd_theme', { dev });
    assert(JSON.stringify(applied) === JSON.stringify(saved), 'applied scene equals saved profile');
    assert(trustedPointers === 3 && trustedKeys >= 6, 'trusted input evidence');
    for (const panel of document.querySelectorAll('[popover]:popover-open')) panel.hidePopover();
    await select(1);
    document.activeElement?.blur();
    window.scrollTo(0, 0);
    await delay(200);
    await invoke('native_smoke_result', {
      keepOpen: true,
      result: {
        ok: true,
        phase,
        dev,
        theme: applied,
        trustedPointers,
        trustedKeys,
        steps: sequence,
        events,
      },
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      keepOpen: true,
      result: {
        ok: false,
        phase,
        events,
        error: String(error),
        stack: error?.stack,
        body: document.body.innerText,
      },
    });
  }
})();
