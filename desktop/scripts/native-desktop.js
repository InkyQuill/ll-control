// Real Tauri settings, GTK tray/menu and private XDG paths. MockBackend only.
(async () => {
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const wait = async (test, label) => {
    for (let i = 0; i < 600; i++) {
      if (await test()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    throw new Error(label);
  };
  const language = window.__nativeDesktopLanguage;
  const labels = {
    'Запускать приложение при входе в систему': 'Start the application at login',
    'Запускать приложение свёрнутым': 'Start minimized',
    Настройки: 'Settings',
    'Приложение и автозапуск': 'Application and autostart',
    'Сворачивать в tray при закрытии окна': 'Minimize to tray when closing the window',
    'Сохранить параметры приложения': 'Save application settings',
  };
  const text = (value) => (language === 'en-US' ? labels[value] : value);
  const button = (name) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === name);
  const click = async (name) => {
    await wait(() => button(name) && !button(name).disabled, name);
    button(name).click();
  };
  const checkbox = (text) =>
    [...document.querySelectorAll('label')]
      .find((l) => l.textContent.trim() === text)
      ?.querySelector('input');
  const set = (label, value) => {
    const input = checkbox(label);
    if (!input) throw new Error(label);
    if (input.checked !== value) input.click();
  };
  try {
    const phase = window.__nativeDesktopPhase;
    let invalidLanguageRejected = false;
    try {
      await invoke('desktop_set_language', { language: 'xx-XX' });
    } catch {
      invalidLanguageRejected = true;
    }
    if (!invalidLanguageRejected) throw new Error('invalid native menu language accepted');
    await click(text('Настройки'));
    [...document.querySelectorAll('summary')]
      .find((s) => s.textContent.trim() === text('Приложение и автозапуск'))
      .click();
    await wait(() => !!checkbox(text('Сворачивать в tray при закрытии окна')), 'desktop form');
    if (phase !== '3')
      await wait(
        async () => (await invoke('desktop_settings_get')).trayReady,
        'registered own tray item',
      );
    if (phase === '1') {
      set(text('Сворачивать в tray при закрытии окна'), true);
      set(text('Запускать приложение свёрнутым'), true);
      set(text('Запускать приложение при входе в систему'), true);
      await click(text('Сохранить параметры приложения'));
      await wait(
        async () => (await invoke('desktop_settings_get')).registration.enabled === true,
        'actual autostart registration',
      );
    }
    await invoke('native_desktop_step', { sequence: Number(phase) });
    const state = await invoke('desktop_settings_get');
    if (phase === '2') {
      if (!state.preferences.startMinimized || !state.trayReady)
        throw new Error('Minimized preference/tray lost on restart');
      set(text('Запускать приложение при входе в систему'), false);
      await click(text('Сохранить параметры приложения'));
      await wait(
        async () => (await invoke('desktop_settings_get')).registration.enabled === false,
        'autostart disable',
      );
    }
    if (phase === '3') {
      if (state.trayReady) throw new Error('No-host fixture unexpectedly ready');
      await wait(
        async () => (await invoke('desktop_settings_get')).trayErrorCode === 'host_unconfirmed',
        'typed native host status',
      );
      const hostMessage =
        language === 'en-US'
          ? 'Tray host registration for this application is unconfirmed.'
          : 'Регистрация приложения в tray host не подтверждена.';
      await wait(
        () => [...document.querySelectorAll('p')].some((p) => p.textContent === hostMessage),
        'localized native host status',
      );
      set(text('Сворачивать в tray при закрытии окна'), false);
      set(text('Запускать приложение свёрнутым'), false);
      set(text('Запускать приложение при входе в систему'), false);
      await click(text('Сохранить параметры приложения'));
      await wait(
        async () => !(await invoke('desktop_settings_get')).preferences.closeToTray,
        'reset isolated preferences',
      );
    }
    await invoke('native_smoke_result', {
      keepOpen: phase !== '3',
      result: {
        ok: true,
        language,
        invalidLanguageRejected,
        hostStatusLocalized: phase === '3',
        phase,
        state: await invoke('desktop_settings_get'),
      },
    });
  } catch (e) {
    await invoke('native_smoke_result', {
      result: { ok: false, error: String(e), stack: e?.stack },
    });
  }
})();
