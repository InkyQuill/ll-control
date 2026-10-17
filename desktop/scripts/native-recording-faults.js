(async () => {
  if (window.__nativeRecordingFaultsStarted) return;
  window.__nativeRecordingFaultsStarted = true;
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const rpc = (method, params = {}) => invoke('daemon_call', {method, params});
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const assert = (condition, why) => { if (!condition) throw new Error(why); };
  const wait = async (predicate, why) => {
    for (let attempt = 0; attempt < 600; attempt++) {
      if (await predicate()) return;
      await delay(50);
    }
    throw new Error(why);
  };
  const button = name => [...document.querySelectorAll('button')].find(element =>
    element.textContent.trim() === name && element.getClientRects().length && !element.closest('[hidden]'));
  const click = async name => {
    await wait(() => button(name) && !button(name).disabled, name);
    button(name).click();
    await delay(0);
  };
  let sequence = 0;
  const step = action => invoke('native_media_step', {sequence: ++sequence, action});
  try {
    const mode = window.__nativeDesktopPhase;
    assert(['stall', 'crash', 'cancel', 'close', 'cleanup', 'cleanup-review', 'cleanup-review-lost'].includes(mode), 'explicit private fault mode');
    const settings = await rpc('get_display_settings');
    const russian = settings.interface_language === 'ru-RU';
    const text = (ru, en) => russian ? ru : en;
    await wait(async () => (await rpc('list_devices')).length === 1, 'one mock device');
    const dev = (await rpc('list_devices'))[0].id;
    await rpc('migrate_screen_collections', {dev});
    await click(text('Экран помпы', 'Pump screen'));
    await click(text('Библиотека', 'Library'));
    const library = () => [...document.querySelectorAll('summary')].find(element =>
      element.textContent.trim() === text('Библиотека роликов', 'Video library'));
    await wait(library, 'studio video library');
    if (!library().parentElement.open) library().click();
    await click(text('Записать экран…', 'Record screen…'));
    await click(text('Выбрать экран или окно', 'Select screen or window'));
    await wait(async () => (await invoke('screen_record_status'))?.phase === 'setup', 'source setup');
    await click(text('Начать запись экрана', 'Start screen recording'));
    await wait(async () => (await invoke('screen_record_status'))?.frames >= 12, 'initial real child recording');
    const initial = await invoke('screen_record_status');
    const before = (await rpc('list_prepared_clips')).map(clip => clip.key);
    // Signal modes use the actual child. Cleanup instead rejects exactly one
    // frontend invoke before delivery; the actual native worker remains owned.
    await step('recording_' + (mode.startsWith('cleanup') ? 'cancel' : mode));
    let retainedReview = null;
    if (mode.startsWith('cleanup')) {
      if (mode.startsWith('cleanup-review')) {
        await click(text('Завершить и просмотреть запись', 'Finish and review recording'));
        await click(text('Подготовить запись целиком', 'Prepare entire recording'));
        await wait(() => document.querySelector(`video[aria-label="${text('Готовый ролик', 'Prepared video')}"]`)?.readyState >= 2,
          'initial prepared review decodes in native WebKit');
        await wait(() => button(text('Сохранить запись в библиотеку', 'Save recording to library'))?.disabled === false,
          'prepared review UI readback');
        retainedReview = await invoke('media_status');
        assert(retainedReview?.phase === 'review_ready', 'actual review lease ready');
      } else {
        await click(text('Приостановить запись', 'Pause recording'));
        await wait(() => !!button(text('Продолжить запись', 'Resume recording')), 'paused UI readback');
      }
      const originalFetch = window.fetch;
      const cancelUrl = window.__TAURI_INTERNALS__.convertFileSrc(mode.startsWith('cleanup-review') ? 'media_cancel' : 'screen_record_cancel', 'ipc');
      let rejected = 0;
      window.fetch = (input, options) => {
        if (String(input) === cancelUrl && rejected++ === 0) {
          const rejection = () => new Response('fixture cleanup invocation rejected', {
            headers: {'Content-Type': 'text/plain', 'Tauri-Response': 'error'}
          });
          if (mode === 'cleanup-review-lost') {
            return originalFetch(input, options).then(async response => {
              assert(response.headers.get('Tauri-Response') === 'ok', 'native cancellation committed');
              await response.arrayBuffer();
              return rejection();
            });
          }
          return Promise.resolve(rejection());
        }
        return originalFetch(input, options);
      };
      try {
        await click(text('Закрыть запись экрана', 'Close screen recording'));
        await wait(() => [...document.querySelectorAll('[role="alert"]')].some(node =>
          node.textContent.includes('fixture cleanup invocation rejected')), 'owned cleanup error visible');
        await delay(1100); // Cross ordinary polling ticks; retain the paused UI.
        if (mode.startsWith('cleanup-review')) {
          assert(button(text('Сохранить запись в библиотеку', 'Save recording to library'))?.disabled,
            'cleanup failure fences Save across polling');
          assert(button(text('Вернуться к редактированию записи', 'Return to recording edits'))?.disabled,
            'cleanup failure fences return to disposed capture');
          const actualReview = await invoke('media_status');
          assert(actualReview.id === retainedReview.id && actualReview.phase === (mode === 'cleanup-review-lost' ? 'cancelled' : 'review_ready'), 'same unpublished native review, with actual cancellation readback');
          assert((await invoke('screen_record_status'))?.phase === 'cancelled', 'capture half of joined cleanup completed');
        } else {
          const resume = button(text('Продолжить запись', 'Resume recording'));
          assert(resume?.disabled, 'cleanup failure fences Resume across polling');
          const retained = await invoke('screen_record_status');
          assert(retained.id === initial.id && retained.phase === 'paused', 'actual worker retained until retry');
          assert(!button(text('Сохранить запись в библиотеку', 'Save recording to library')), 'no publication action');
        }
        assert((await rpc('list_prepared_clips')).every(clip => before.includes(clip.key)), 'failed cleanup never publishes');
        assert(rejected === 1, 'cleanup never retried automatically');
      } finally {
        window.fetch = originalFetch;
      }
    }
    if (mode === 'close') {
      await invoke('native_smoke_result', {result: {ok: true, close_while_stopped: true,
        mode, language: settings.interface_language, first_id: initial.id}, keepOpen: true});
      return;
    }
    let failure = null;
    if (mode === 'stall' || mode === 'crash') {
      await wait(async () => (await invoke('screen_record_status'))?.phase === 'failed', 'failed child surfaced');
      failure = (await invoke('screen_record_status')).message;
      const explanation = mode === 'stall'
        ? text('Процесс записи не отвечает. Запись остановлена. Выберите источник заново.',
          'The recording process is not responding. Recording stopped. Select the source again.')
        : text('Процесс записи завершился. Выберите источник заново.',
          'The recording process exited. Select the source again.');
      await wait(() => [...document.querySelectorAll('[role="alert"]')].some(node =>
        node.getClientRects().length && node.textContent.trim() === explanation), 'localized recovery explanation');
      assert(!button(text('Сохранить запись в библиотеку', 'Save recording to library')), 'failed recording cannot be saved');
      assert((await rpc('list_prepared_clips')).every(clip => before.includes(clip.key)), 'failure never publishes');
      await step('recording_reaped');
    }
    await click(text('Очистить запись…', 'Clear recording…'));
    await click(text('Подтвердить очистку записи', 'Confirm clearing the recording'));
    await wait(() => !!button(text('Выбрать экран или окно', 'Select screen or window')), 'clear restores source choice');
    if (mode === 'cancel' || mode.startsWith('cleanup')) await step('recording_reaped');
    await click(text('Выбрать экран или окно', 'Select screen or window'));
    await wait(async () => (await invoke('screen_record_status'))?.phase === 'setup', 'new source after failure');
    const recovered = await invoke('screen_record_status');
    assert(recovered.id !== initial.id && recovered.frames === 0, 'fresh capture identity and time budget');
    await click(text('Начать запись экрана', 'Start screen recording'));
    await wait(async () => (await invoke('screen_record_status'))?.frames >= 12, 'new worker produces frames');
    await click(text('Завершить и просмотреть запись', 'Finish and review recording'));
    await click(text('Подготовить запись целиком', 'Prepare entire recording'));
    await wait(() => document.querySelector(`video[aria-label="${text('Готовый ролик', 'Prepared video')}"]`)?.readyState >= 2,
      'recovered recording decodes in native WebKit');
    assert((await rpc('list_prepared_clips')).every(clip => before.includes(clip.key)), 'review still requires explicit Save');
    await click(text('Сохранить запись в библиотеку', 'Save recording to library'));
    await wait(async () => (await invoke('media_status'))?.phase === 'complete', 'explicit recovered publication');
    const saved = (await invoke('media_status')).key;
    await wait(async () => (await rpc('get_screen_collections', {dev})).collections?.backgrounds.some(item =>
      item.kind === 'video' && item.key === saved), 'recovered collection membership');
    await click(text('Закрыть запись экрана', 'Close screen recording'));
    await wait(async () => (await invoke('screen_record_status'))?.phase === 'cancelled', 'recovered capture released');
    await step('recording_recovered');
    await invoke('native_smoke_result', {result: {ok: true, mode, language: settings.interface_language,
      first_id: initial.id, recovered_id: recovered.id, failure, explicit_save: true,
      native_decode: true, cleanup_retry: mode.startsWith('cleanup'), retained_review_id: retainedReview?.id ?? null, lost_cancel_reply: mode === 'cleanup-review-lost', saved_key: saved, collection_membership: true}, keepOpen: true});
  } catch (error) {
    await invoke('native_smoke_result', {result: {ok: false, error: String(error), stack: error?.stack,
      alerts: [...document.querySelectorAll('[role="alert"]')].map(node => node.textContent)}});
  }
})();
