(async () => {
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const [mode, language, sealed] = window.__nativeDesktopPhase.split(':');
  const ru = language === 'ru-RU';
  const wait = async (test, label) => {
    for (let n = 0; n < 400; n++) {
      if (await test()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    throw new Error(label);
  };
  const assert = (test, label) => {
    if (!test) throw new Error(label);
  };
  try {
    const navigation = ru ? 'Настройки' : 'Settings',
      title = ru ? 'Сохранённое состояние прошивки' : 'Saved firmware state';
    if (sealed !== 'sealed')
      await wait(
        () => document.body.textContent.includes(ru ? 'Daemon подключён' : 'Daemon connected'),
        'ordinary connection',
      );
    await wait(
      () =>
        [...document.querySelectorAll('button')].some((b) => b.textContent.trim() === navigation),
      'settings navigation',
    );
    [...document.querySelectorAll('button')]
      .find((b) => b.textContent.trim() === navigation)
      .click();
    const card = () =>
      [...document.querySelectorAll('section')].find((e) => e.getAttribute('aria-label') === title);
    await wait(() => card(), 'saved recovery card');
    assert(
      card().textContent.includes(ru ? 'ещё не прочитано' : 'has not been read yet'),
      'must not automatically inspect',
    );
    const button = card().querySelector('button');
    assert(!button.disabled, 'read must be available without an ordinary device');
    button.click();
    await wait(() => card().querySelector('textarea')?.value, 'read-only snapshot');
    const snapshot = JSON.parse(card().querySelector('textarea').value),
      expected = await rpc('get_firmware_recovery');
    for (const key of ['state', 'operation', 'guards', 'diagnostic'])
      assert(
        JSON.stringify(snapshot[key]) === JSON.stringify(expected[key]),
        'raw snapshot matches saved ' + key,
      );
    assert(
      snapshot.state ===
        {
          none: 'no_saved_operation',
          guarded: 'guarded',
          guard_only: 'guard_only',
          unreadable: 'unreadable',
        }[mode],
      'expected recovery classification',
    );
    assert(card().querySelectorAll('button').length === 1, 'read is the only operation');
    assert(
      card().textContent.includes(ru ? 'а не текущее состояние операции' : 'not live status'),
      'saved evidence boundary',
    );
    if (snapshot.operation) {
      assert(card().textContent.includes(snapshot.operation.operation_id), 'operation ID');
      assert(card().textContent.includes('0416:7395'), 'bound target');
    }
    if (mode !== 'none')
      assert(card().querySelector('[role="alert"]'), 'unresolved state announced');
    await invoke('native_smoke_result', {
      result: {
        ok: true,
        mode,
        language,
        sealed: sealed === 'sealed',
        state: snapshot.state,
        no_device: true,
        read_only: true,
        explicit_read: true,
        raw_readback: true,
        guard_count: snapshot.guards.length,
      },
      keepOpen: true,
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      result: { ok: false, error: String(error), stack: error?.stack },
    });
  }
})();
