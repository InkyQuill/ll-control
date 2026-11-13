// Private WebKitGTK/FileChooser/Unix IPC/MockBackend acceptance. No real USB.
(async () => {
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args),
    rpc = (method, params = {}) => invoke('daemon_call', { method, params });
  const wait = async (test, label) => {
    for (let i = 0; i < 600; i++) {
      if (await test()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    throw new Error(label);
  };
  const assert = (ok, why) => {
    if (!ok) throw new Error(why);
  };
  const english = window.__nativeDesktopLanguage === 'en-US';
  const text = (en, ru) => (english ? en : ru);
  const button = (name) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === name);
  const click = async (name) => {
    await wait(() => button(name) && !button(name).disabled, name);
    button(name).click();
  };
  const field = (label, root = document) =>
    [...root.querySelectorAll('label')]
      .find((l) => l.textContent.trim().startsWith(label))
      ?.querySelector('input,select');
  const set = (input, value) => {
    assert(input, 'native form field');
    input.value = value;
    input.dispatchEvent(
      new Event(input.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }),
    );
  };
  const phase = async (name) => (await invoke('group_archive_status'))?.phase === name;
  const open = async () => {
    await click(text('Settings', 'Настройки'));
    const summary = [...document.querySelectorAll('summary')].find(
      (s) =>
        s.textContent.trim() === text('Settings group backup', 'Резервная копия группы настроек'),
    );
    if (!summary.parentElement.open) summary.click();
  };
  try {
    await wait(async () => (await rpc('list_devices')).length === 1, 'private mock device');
    const dev = (await rpc('list_devices'))[0].id;
    if (window.__nativeWindowsRecover) {
      const pending = JSON.parse(localStorage.getItem('llcontrol.group.pending'));
      assert(
        pending?.mode === 'windows' && pending.dev === dev,
        'Windows operation ID survives desktop restart',
      );
      assert((await invoke('group_archive_status')) === null, 'no implicit new source worker');
      const before = await rpc('list_profiles');
      await open();
      await click(text('Read group result', 'Прочитать результат группы'));
      await wait(() => phase('complete'), 'cold receipt readback');
      const status = await invoke('group_archive_status');
      assert(
        status.readback && status.readback.operation_id === pending.id,
        'original receipt recovered',
      );
      assert(
        !document.querySelector('.windows-bindings'),
        'no file grant reconstructed from localStorage',
      );
      assert(
        JSON.stringify(before) === JSON.stringify(await rpc('list_profiles')),
        'cold readback has no commit',
      );
      await invoke('native_smoke_result', {
        keepOpen: true,
        result: { ok: true, dev, operation: pending.id, library: before, cold_readback: true },
      });
      return;
    }
    await rpc('patch_lcd_config', { dev, patch: { brightness_percent: 64, rotation_degrees: 90 } });
    let library = await rpc('list_profiles');
    await rpc('edit_profile', {
      expected_revision: library.revision,
      edit: {
        action: 'save',
        profile: {
          slot: 2,
          name: 'Keep Linux Windows slot',
          content: { kind: 'fan', value: { mode: 'fixed', percent: 42 } },
        },
      },
    });
    const before = await rpc('list_profiles'),
      lcd = (await rpc('get_lcd_state', { dev })).requested;
    await click(text('Pump screen', 'Экран помпы'));
    await wait(() => !!field(text('Name', 'Название')), 'LCD draft editor');
    set(field(text('Name', 'Название')), 'Keep Windows local draft');
    await open();
    set(field(text('Settings group', 'Группа настроек')), 'fan_and_lighting');
    const choose = text('Choose Windows ZIP…', 'Выбрать Windows ZIP…');
    await click(choose);
    await wait(() => phase('bindings'), 'native chosen ZIP inspection');
    await click(text('Cancel Windows preparation', 'Отменить Windows подготовку'));
    await wait(() => phase('cancelled'), 'cancel source before preparation');
    await click(text('Close group details', 'Закрыть сведения о группе'));
    assert(
      JSON.stringify(before) === JSON.stringify(await rpc('list_profiles')),
      'source cancel leaves profile library unchanged',
    );
    await click(choose);
    await wait(() => phase('bindings'), 'fresh Windows file grant');
    await wait(() => !!document.querySelector('.windows-bindings'), 'source binding form rendered');
    const area = document.querySelector('.windows-bindings');
    set(field(text('Windows source controller', 'Исходный контроллер Windows'), area), '1');
    const ids = [
      'explicit:CPULoad',
      'fixture:cpu',
      'explicit:GPULoad',
      'explicit:GPUTemperature',
      'explicit:PumpRPM',
      'explicit:CoolantTemp',
    ];
    const inputs = [...area.querySelectorAll('.windows-sources input')];
    assert(inputs.length === 6, 'six explicit source roles');
    inputs.forEach((input, i) => set(input, ids[i]));
    await click(text('Review Windows settings', 'Проверить настройки Windows'));
    await wait(() => phase('prepared'), 'native normalized preview');
    const prepared = await invoke('group_archive_status');
    assert(
      prepared.windows_lighting.fan_modes.length === 6 &&
        prepared.windows_lighting.pump_modes.length === 16,
      'complete lighting review',
    );
    assert(
      prepared.summary.cooling_modes.fan.length === 8 &&
        prepared.summary.cooling_modes.pump.length === 3,
      'complete cooling review',
    );
    assert(
      !JSON.stringify(prepared).includes('DynamicSettings'),
      'private source report stays outside webview',
    );
    assert(
      JSON.stringify(before) === JSON.stringify(await rpc('list_profiles')),
      'preview has no commit',
    );
    const apply = text('Import and apply', 'Импортировать и применить');
    set(inputs[0], 'explicit:NewCPULoad');
    await wait(() => button(apply)?.disabled, 'changed bindings fence Apply');
    await click(text('Review Windows settings', 'Проверить настройки Windows'));
    await wait(async () => {
      const s = await invoke('group_archive_status');
      return (
        s?.phase === 'prepared' &&
        s.windows_options?.bindings.CPULoad.sensor_id === 'explicit:NewCPULoad'
      );
    }, 'fresh native preview for edited binding');
    await click(apply);
    await wait(() => phase('complete'), 'explicit native Windows commit');
    const result = await invoke('group_archive_status'),
      operation = result.id;
    assert(result.readback?.selected_content_current, 'storage commit/readback confirmed');
    assert(
      !result.readback.runtime.recovery_required && !result.readback.runtime.last_error,
      'runtime reconciled',
    );
    const bank = await rpc('get_cooling_modes', { dev }),
      cooling = await rpc('get_cooling_state', { dev }),
      after = await rpc('list_profiles');
    assert(bank.fan.length === 8 && bank.pump.length === 3, '8/3 modes stored');
    assert(
      cooling.fan.mode_id === 'UserDefine2' && cooling.pump.mode_id === 'PWMQuiet',
      'source selected modes applied',
    );
    assert(after.revision === before.revision + 1, 'one profile transaction');
    assert(
      after.profiles.find((p) => p.content.kind === 'fan' && p.slot === 2)?.name ===
        'Keep Linux Windows slot',
      'Linux extension slot retained',
    );
    assert(
      JSON.stringify((await rpc('get_lcd_state', { dev })).requested) === JSON.stringify(lcd),
      'Screen config preserved',
    );
    await click(text('Read group result', 'Прочитать результат группы'));
    await wait(() => phase('complete'), 'explicit readback');
    assert(
      JSON.stringify(after) === JSON.stringify(await rpc('list_profiles')),
      'readback never reimports',
    );
    await click(text('Reapply saved group', 'Повторить применение сохранённой группы'));
    await wait(() => phase('complete'), 'explicit group reapply');
    assert(
      JSON.stringify(after) === JSON.stringify(await rpc('list_profiles')),
      'reapply makes no second commit',
    );
    await click(text('Pump screen', 'Экран помпы'));
    assert(
      field(text('Name', 'Название')).value === 'Keep Windows local draft',
      'local LCD draft retained',
    );
    await invoke('native_smoke_result', {
      keepOpen: true,
      result: {
        ok: true,
        dev,
        operation,
        library: after,
        lcd,
        bank,
        source_cancel: true,
        preview_no_commit: true,
        changed_binding_fence: true,
        explicit_import: true,
        source_selection: true,
        reapply_no_commit: true,
        preserved_draft: true,
      },
    });
  } catch (error) {
    await invoke('native_smoke_result', {
      result: { ok: false, error: String(error), stack: error?.stack },
    });
  }
})();
