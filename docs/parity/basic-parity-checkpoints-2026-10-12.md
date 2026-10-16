> Исторические checkpoints до сверки 2026-10-12. Не активный план: ранние открытые пункты могут быть закрыты позднейшими результатами. Актуальные остатки — [чеклист](basic-parity-checklist.md).

# Базовый parity: единый чеклист приёмки

Актуальная очередь — [roadmap](../../roadmap.md), незакрытые крупные области —
[baseline gaps](lcdv2-baseline-gaps.md). Ниже сохраняются проверки отдельных
срезов; более поздние результаты уточняют ранние checkpoints.

**Сверка Screen reset, 2026-10-12:** реализация storage/actor/IPC/GUI и
коллекций завершена. Native keyboard/cancel/commit/LCD failure/reapply/AT-SPI,
mock reset → sleep/reattach/restart и sleep во время committed reset проверены.
Остаются native ambiguous reply, expanded scaling/Wayland/speech, полные
assertions сохранности draft/video/fonts, прямые in-flight detach/shutdown
и аппаратная приёмка. См. последние секции и
[native evidence](evidence/2026-10-12-screen-reset-native.json).


Уже выполненные компонентные проверки перечислены в
[сводке реализации](implementation-status.md); ниже — итоговая сквозная приёмка.
Каждый пункт требует отдельного результата, даты, версии кода, окружения
и evidence. Пустой пункт означает «не проверен»; software/mock не заменяет
наблюдение железа. Реальные тесты отложены пользователем.

## Software acceptance

- [x] Общий frontend checkpoint: check/build,26 cross-language contracts,87 browser tests и release workspace build прошли; native retry прошёл после диагностических дополнений. [Evidence и исходный timeout](evidence/2026-10-11-combined-software-checkpoint.json). Это не закрывает финальную приёмку; [readback oracle уточнён и проверен](profile-persistence-acceptance.md), точный trace первого timeout отсутствует.

- [x] Внешние cooling/thermal/dynamic RGB updates: daemon status обновляется без замены draft; explicit Load использует новый state; смена RGB base блокирует несовместимый Apply; автоматических set-записей нет. [Browser matrix и границы](external-control-acceptance.md).

- [x] Native aged stale и hot→cool: публикация cache останавливается, age>3s отзывает значение; cooling fallback/thermal retention/dynamic suppression и восстановление свежим sample без Apply. [Evidence](evidence/2026-10-11-native-aged-consumers.json). Software transport independence [проверена](transport-consumer-acceptance.md); физическое recovery остаётся.

- [x] Native shared-source missing/recovery: pump+fan fallback, thermal priority, dynamic suppression/binding, draft/theme preservation и recovery без повторного Apply curves. [Evidence](evidence/2026-10-11-native-consumer-recovery.json). Aged stale и software transport independence проверены отдельными сценариями ниже.

- [x] Browser reflow320/640: пять страниц, text layer, curve и thermal rules без горизонтальной прокрутки; controls остаются в viewport. [Evidence](evidence/2026-10-11-desktop-reflow.json). Native zoom/OS text scaling остаются.

- [x] Browser keyboard settings/cooling/lighting/sources/layers, visible control names и field boundary contrast; 7 tests, check/build прошли. [Границы и оставшаяся accessibility matrix](accessibility-acceptance.md). Native keyboard и AT-SPI проверены последующими отдельными checkpoints ниже; speech/zoom остаются открытыми.

- [x] Native source editor: i915/xe engine и VRAM, изоляция GPU, readable labels,
  числовые значения, сохранение и restart/readback обоих bindings; recommended
  reset не назначает engine автоматически общим GPU load.
  [Evidence](evidence/2026-10-10-native-intel-sources.json).


- [x] xe ENGINES query + per-engine active/total PMU: fixtures topology/format,
  warmup/reset/stale/permission/recovery, multi-GPU, helper→cache→LCD formatting;
  i915/xe engine можно выбрать явно, recommended не выдаёт его за общий load.
  [Контракт](intel-engine-utilization.md).


- [x] Intel i915/xe dedicated VRAM query: bounded helper, per-device identity,
  redacted zero без fake usage, SYSMEM исключён, timeout/reap/recovery;
  synthetic collector→cache→IPC→JPEG и stale. [Контракт](intel-memory-query.md).


- [x] ASUS EC CPU voltage: source-confirmed asusec in0/mV→V, CPU role/recommendations, stable identity/renumber/missing/recovery и invalid/stale; synthetic cache→IPC→LCD preview. [Evidence](evidence/2026-10-10-asus-cpu-voltage.json). Наличие и accuracy на реальной плате не проверены; остальные motherboard voltage providers открыты.

- [x] Native theme Apply/clear с потерей успешного ответа: changed gradient и полная тема приняты ровно один раз; late error не даёт false success, последующие draft edits и profile library сохраняются. Explicit readback/Apply восстанавливают workflow; clear даёт null theme/Idle. [Evidence](evidence/2026-10-10-native-theme-lost-reply.json). Preparation/cancellation и frame barriers [сопоставлены и проверены](theme-preparation-order.md).

- [x] Display interrupted replies: browser reconnect после принятого Apply/clear или начатого Load; поздние success/error не меняют новый status/draft, команды не повторяются, явный readback показывает принятую тему. [Evidence](evidence/2026-10-10-display-interrupted.json). Это browser transport fixture; native Apply/clear после commit также проверены (строка выше), late Load остаётся browser-only.

- [x] Native restart с missing image/font: Idle + restore error + null active theme, отсутствие mock JPEG, сохранность device settings/profile/draft; возврат bytes и restart восстанавливают тему без Apply. [Evidence](evidence/2026-10-10-native-restart-dependencies.json), [сценарии](restart-dependency-acceptance.md). Исправлен IPC decoder для законного result:null.

- [x] Optional NVIDIA voltage: query-only flags, explicit session, UUID/units/zero/invalid, timeout/reaping, multi-GPU failure/reorder/recovery, source candidates и cache→LCD text/stale проверены fixtures. [Evidence](evidence/2026-10-10-nvidia-voltage.json). Реальный driver, GUI и Wayland/headless support этим не подтверждены.

- [x] Intel hwmon voltage: i915 in0 / xe in1, mV→V, GPU membership, renumber/missing/zero/disabled/malformed/stale и cache→IPC→LCD preview проверены fixtures. [Evidence](evidence/2026-10-10-intel-voltage.json). Hardware accuracy и доступность на конкретной GPU не подтверждены.

- [x] Source inventory Resource/Settings: восстановлены versioned XAML, сопоставлены реальные bindings/ranges/visibility и шесть CPU source selectors. [Evidence](evidence/2026-10-10-system-info-xaml.json). Это source audit, не реализация всех providers или Windows runtime observation.
- [x] CPU fan source: явный выбор hwmon RPM в GUI, Apply-only, рекомендации без автоматического fan binding, backend zero/missing/recovery/reload, native value/profile-independent persistence и daemon restart. [Evidence](evidence/2026-10-10-cpu-fan-mapping.json). Wiring/accuracy требуют аппаратной проверки; LCDV2 fan не считается CPU fan.

- [x] Native Display smoke: import → preview → DOM load/edit/Apply → synthetic sensor update → GUI close → daemon restart. [Результат 2026-10-10, окружение и границы](native-desktop-smoke.md). Это узкий сценарий, остальные строки ниже остаются самостоятельной приёмкой.

- [x] Native control happy paths: Sensors, pump/fan Apply, LCD field patches, manual/thermal/dynamic RGB и restart с восстановлением applied. [Результат и границы 2026-10-10](native-desktop-smoke.md).

- [x] Schema preservation: incompatible/corrupt/unknown-field device settings, unsupported embedded theme, symlink и oversized files отклоняются read/write с сохранением исходных bytes. Legacy device v1 без version читается без переписывания; явная запись сохраняет прежние настройки. [Политика](schema-compatibility.md), [evidence](evidence/2026-10-10-schema-protection.json). Это не future-version migration engine.

- [x] Сохранение: JSON-RPC Apply отделён от durable save; disk error/retry, успешный и неуспешный final flush, replay brightness/rotation и browser pending/error/saved проверены. [Сценарии и ограничения](persistence-acceptance.md), [evidence](evidence/2026-10-10-persistence-acceptance.json). Ошибка создания temp file; не power-loss или native disk-fault acceptance.

- [ ] Полная цепочка fmt / strict Clippy / tests / docs / release build на итоговом состоянии.
- [x] Текущий единый checkpoint после Cooling validation:612Rust/113browser/26contracts, fmt/strict Clippy/docs/release/frontend, native400steps/12AT-SPI и полный smoke; source hashes до/после совпали. [Evidence](evidence/2026-10-11-integrated-after-cooling-validation.json). Финальный gate после закрытия оставшихся требований сохраняется.
- [x] IPC software: malformed/oversized inputs, неизвестная версия/поля темы, недопустимые цвета/геометрия/assets отклоняются без замены активной темы/generation; persisted theme и restart проверены. [Сценарии и границы](schema-compatibility.md#отклонение-ipc-запросов-при-активной-теме--2026-10-11). Физическая картинка отдельно.
- [x] GPU hwmon power: input/average отдельны, microWatts→Watts, disabled/invalid/zero, renumber/disappearance; fixture→cache→IPC→LCD preview fresh/stale. [Evidence](evidence/2026-10-10-gpu-hwmon-power.json). Hardware verification остаётся.
- [x] NVIDIA power.draw: typed Watts, UUID binding, valid zero/unavailable/invalid, bounded query arguments и collector cache проверены fixtures. [Evidence](evidence/2026-10-10-nvidia-power.json). Реальный driver не проверен.
- [x] AMDGPU voltage/clocks: ABI units, отдельные rails, stable IDs, errors/zero, cache→IPC→LCD и GUI Sensors→Duo→Apply проверены fixtures. [Evidence](evidence/2026-10-10-amdgpu-voltage-clocks.json). Другие providers и hardware accuracy остаются.
- [x] CPUFreq: related_cpus identity, policy renumber/offline/topology change, отдельные hardware current/average и driver sources, bounds/errors/zero, cache→IPC→MHz/JPEG preview проверены fixtures. [Evidence](evidence/2026-10-10-cpufreq.json). Hardware accuracy отложена.
- [x] NVIDIA current graphics/memory clocks: MHz→Hz→MHz, UUID identity, независимые errors/unsupported/zero, query args и collector cache→LCD text проверены fixtures. [Evidence](evidence/2026-10-10-nvidia-clocks.json). Реальный драйвер не проверен.
- [x] Intel i915/xe clocks: actual/requested, GT/tile/legacy isolation, stable physical IDs, malformed/missing/zero, bounds и cache→IPC→LCD preview проверены fixtures. [Evidence](evidence/2026-10-10-intel-clocks.json). Это не проверка utilization/VRAM или аппаратной accuracy.
- [x] Network RX/TX: deltas/units, warmup/reset/errors/rename/reconnect, partial totals, virtual identity, bounds и GUI→theme binding проверены fixtures. [Evidence](evidence/2026-10-10-network-rates.json). Hardware accuracy и перечисление интерфейсов Windows/Linux не приравниваются.
- [x] Storage: reserved/free/available, volume identity, deduplication, malformed/empty/recovery, bounded helper/retained child, collector→LCD text и GUI binding проверены fixtures. [Evidence](evidence/2026-10-10-storage-capacity.json). Реальные FS и Windows enumeration equivalence не проверены.
- [x] Browser global source editor: GPU membership options, local draft, navigation/reconnect retention, Apply-only persistence, external revision conflict и clear-draft без записи. [Evidence](evidence/2026-10-10-sensor-defaults-ui.json). Native, recommended reset и lost-reply acceptance зафиксированы отдельными строками ниже.
- [x] Native global GPU sources: DOM GPU B choice/filter, Apply/persistence, clear draft без записи, сохранение draft через navigation/reconnect, explicit reload и restart config; LCD theme не изменяется. [Evidence](evidence/2026-10-10-native-sensor-defaults.json). CPU selection, recommended reset и lost-reply acceptance зафиксированы ниже.
- [x] Рекомендуемый выбор CPU/GPU: только fresh valid candidates, один GPU, actual clock priority; reset только draft, native CPU/GPU Apply/values/reconnect/restart, explicit LCD theme не меняется. [Evidence](evidence/2026-10-10-recommended-sensor-defaults.json). Linux selection policy не приравнивается к HWiNFO.
- [x] Native source lost reply: daemon сохраняет revision2, proxy теряет ответ, GUI показывает error без success, сохраняет draft и блокирует stale Apply; explicit reload → Apply сохраняет revision3. Proxy: 1 target request/1 drop, всего 3 записи (initial/fault/recovery), restart сохраняет config. LCD theme и requested cooling/RGB snapshots не изменены. [Evidence](evidence/2026-10-10-native-source-lost-reply.json).
- [x] Датчики: CPU/GPU/RAM/coolant/RPM, units, стабильные ID, missing/invalid/stale без fake zero и без подмены другим датчиком. [Software контракт и matrix](sensor-consumer-acceptance.md#общий-software-контракт-readings--lcd--2026-10-11): provider/IPC/identity regressions и mixed host/device JPEG. Intel aggregate, дополнительные voltage providers и hardware accuracy остаются отдельными открытыми пунктами.
- [x] i915 engine PMU software fixtures: card renumber/reconnect, denied/open/read failure, cpumask migration, event validation, stale/reset и release handles. Это не проверка kernel PMU на GPU.
- [ ] Intel: PMU/query permission failure и отсутствие dedicated VRAM дают unavailable; i915 без достоверного accounting не показывает fake used=0; card renumbering не меняет physical binding, engine aggregation явно определена.
- [x] Cycle software: interval boundaries/loop, согласованные label/value/unit с разными стилями, missing/stale/recovery, reorder/remove и restart. После отключения IPC connection daemon продолжает отправлять обе полные JPEG sequences; [матрица и границы](cycle-acceptance.md). Отдельный native Cycle close и физический вывод не проверены.
- [x] BackgroundColor software: horizontal/vertical/radial gradients, два цвета/Flip, независимые настройки типов, legacy solid, renderer pixels/overlay, editor draft/Apply/load и native profile/restart. [Evidence](evidence/2026-10-10-gradient-backgrounds.json). Непрерывный raster не объявляется pixel-identical p5.
- [x] CSS em sizing: подтверждённый Windows px contract, Noto79/80px wrap regression, независимый Chromium80px measurement; [изменение и границы](text-em-sizing.md). Shaping/raster остаются открытыми.
- [x] Text line boxes: optional half-leading, top/bottom/center anchors, negative leading и blank lines, legacy default; native edit/load/profile/restart. [Evidence](evidence/2026-10-10-text-line-box.json). Это не полная browser font/glyph equivalence.
- [x] Noto Sans TC Regular/Bold: pinned licensed assets, полные cmap20745, реальные glyph outlines для TC sample, отличающиеся веса, воспроизводимая подготовка, native font edit/profile/restart. [Evidence](evidence/2026-10-10-noto-sans-tc.json). Исторический checkpoint для прежних TTF; три новых bundled fonts проверены ниже. Замена Noto OTF проверена отдельной строкой ниже.
- [x] Bundled Abril Fatface/Caprasimo/Cubic11: pinned official bytes и лицензии, glyph/enum-to-font render equality,26 frontend contracts, native DOM selection/Apply/profile/restart. [Evidence](evidence/2026-10-10-display-fonts.json). Не доказывает pixel-identical Windows typography; Noto OTF проверен отдельной строкой ниже.
- [x] Noto OTF2.002: cmap20748, все mapped horizontal metrics/outlines совпадают с Windows; восстановлены U+2252/U+25C8/U+30EDE. Offline reproduction, обе glyph regressions, RPC upload/preview и native bundled layer Apply/profile/restart прошли. [Evidence](evidence/2026-10-10-noto-coverage.json). Не доказывает pixel-identical layout/raster.
- [x] User font foundation: bounded validator и exact-byte store, unique keys/reopen, corruption/path/symlink/oversize rejection и failed-root retry. [Evidence](evidence/2026-10-10-font-store.json). Это не готовый import workflow.
- [x] Font upload RPC: полный CJK font передан chunks по64KiB, exact bytes сохранены; offset/size/capacity/expiry, identical retry, повторный Finish и cancel без удаления asset проверены. [Evidence](evidence/2026-10-10-font-upload.json). Это backend workflow без GUI и theme references.
- [x] Managed font backend: strict key/legacy schema, text/clock/sensor/cycle/parts rendering,64MiB total/dedup, full CJK upload→preview→profile; corrupt Apply/save не меняют active/library, restart работает и сохраняет intent при missing font. Подготовленный runtime переживает удаление файла. [Evidence](evidence/2026-10-10-theme-fonts.json). Проверено с MockBackend, не native GUI/железом.
- [x] User font GUI: browser bounded chunks, cancel/unmount, lost-Finish Status recovery, no implicit Apply и builtin selection снимает managed reference. Native WebKitGTK File→RPC→font selection→Apply→profile/restart, GUI close/reconnect сохраняют тему. [Evidence](evidence/2026-10-10-font-gui.json).
- [x] Font recovery: browser held chunk→disconnect→cancel/reconnect/retry и invalid Finish→retry сохраняют draft без auto Apply. Native invalid File→error→valid retry, sensor label/unit font selection→Apply→profile/restart прошли. [Evidence](evidence/2026-10-10-font-recovery.json).
- [x] Font cleanup: общий session store для import/load/replay, offline/profile refs, draft keys, fail-closed corrupt settings/library и строгие filenames. RPC и browser scan/delete/изменение draft проверены; native GUI удаляет prior-session orphan и сохраняет font workflow/restart. [Evidence](evidence/2026-10-10-font-cleanup.json).
- [x] Native font lost Finish: proxy теряет успешный ответ, GUI читает Status той же сессии и получает сохранённый asset без повторного upload; один файл, Apply/profile/restart проходят. [Evidence](evidence/2026-10-10-native-font-lost-finish.json).
- [x] Native partial font upload: socket обрывается после первого принятого64KiB chunk; нет следующих chunks/Finish, Cancel освобождает сессию без asset. GUI error/retry сохраняет draft и active theme; явный retry, lost Finish и restart проходят. [Evidence](evidence/2026-10-10-native-font-partial-upload.json).
- [x] Native saved dependencies: внешний missing/corrupt font и image сохранённого профиля; DOM Apply error без success, draft/library/revision/active theme сохранены, возврат exact bytes → успешный Apply. [Evidence](evidence/2026-10-10-native-saved-dependencies.json). Restart проверен после восстановления файлов; это не restart с отсутствующим dependency.
- [x] Font cleanup concurrency/partial failure: 32 shared-store load/import/delete races сохраняют успешные load/import; injected unlink failure отделён от успешного удаления, повтор очищает остаток. Browser показывает failed key и явный rescan/retry. [Evidence](evidence/2026-10-10-font-cleanup-races.json). Stress не доказывает все расписания; внешний filesystem writer не синхронизирован store lock.
- [x] Темы: background, image fit/crop/contain/contain_native/stretch/opacity, Unicode text, word wrap/blank lines/graphemes/vertical anchors, sensor values, clock; порядок слоёв, clipping, °C/°F и все доступные настройки. [Software matrix и границы](theme-controls-acceptance.md). Windows locale/raster, общая GUI accessibility, M4 effects и физический вывод остаются отдельными требованиями.
- [x] Static image software: PNG/JPEG/WebP/BMP, corrupt/truncated/oversized inputs и decode limits; EXIF1…8 до normalization/fit, без двойного поворота. Preview, protected deletion и recovery сопоставлены с существующими tests. [Матрица и ограничения](image-import-acceptance.md). Физический вывод и отдельный native EXIF import остаются непроверенными.
- [x] Render loop: daemon-owned обновление без GUI и fresh/missing/stale snapshots сопоставлены с integration/native checkpoints; actual CPU raster/JPEG не блокирует same-stream State/sensors/PWM/RGB/Clear, autonomous curve и mock HID writes. [Полный software критерий и измерения](theme-preparation-order.md#реальный-cpu-rasterjpeg-и-общий-render-loop-критерий--2026-10-11). Один debug-run, не hardware SLA.
- [x] Browser preview: late image/error после draft edit, device change и reconnect отбрасываются; текущая ошибка видна, explicit retry восстанавливает preview без device writes. Single-flight и live stop/coalescing проверены. [Evidence](evidence/2026-10-11-preview-recovery.json).
- [x] Actor ordering подготовки темы: новая заявка отменяет старую до готовности assets; Clear/новый источник не могут быть отменены поздним завершением старой подготовки. Rejected job не отправляет frames и не меняет settings; новый явный Apply работает. [Контракт и границы](theme-preparation-order.md), [evidence](evidence/2026-10-10-theme-preparation-order.json).
- [x] Software Apply/replace/clear: actor tokens и batch barriers не допускают поздних frames/settings; held real asset I/O + Clear/image replacement проверены, ошибка подготовки сохраняет рабочую тему, transport error отделена. [Консолидированная matrix](theme-preparation-order.md). Физический decoder resync не подтверждён.
- [x] Persistence: theme/assets/bindings переживают restart, invalid/missing assets видны; данные другой physical identity не применяются. [Сводное доказательство и границы](restart-dependency-acceptance.md#corrupt-dependency-при-restart--2026-10-11): corrupt PNG/font → error/no JPEG → repair/restart, native missing dependencies, serial replay, schema/disk failures.
- [x] Cooling: fan/pump presets/custom curves, RPM mapping, hysteresis, fallback, manual override, motherboard source и replay. [Полная software матрица доказательств](lcdv2-m2-cooling-acceptance.md#полный-software-cooling-критерий--2026-10-11). Физические duty/RPM/source и native accessibility остаются отдельными gates.
- [x] Motherboard pump80% → restart → explicit MCU67% → restart: policy cancellation, cmd138 source/duty и durable settings; fan73% applied также переживает restart. Исправлен raw PWM replay, обходивший cooling state. [Сценарий и границы](persistence-acceptance.md#cooling-source-и-ручной-pwm-после-restart--2026-10-11).
- [x] RGB runtime software: manual/direct ordering, sensor-driven arithmetic/bindings, thermal priority/10s grace, retained-prefix restore, retry/partial failure и durable policy. [Assertions и границы](rgb-software-acceptance.md):27 targeted tests passed.
- [ ] Полный RGB parity: завершить Windows mode-specific controls/sync/LED loops catalog и подтвердить правильное физическое покрытие LED. Runtime software принят отдельно; known direct prefix не равен полной зоне.
- [x] Native Display reconnect: остановка fixture daemon при открытом GUI → offline и disabled Apply/load → restart → сохранённый draft, replay прежней темы и успешный explicit load/Apply. Это реальный Unix IPC через WebKitGTK с MockBackend, не USB hotplug; остальные страницы и in-flight mutations остаются. [Evidence](evidence/2026-10-10-native-display-reconnect.json).
- [x] Native reconnect остальных страниц: Cooling PWM, Settings brightness/rotation, Lighting manual/thermal/dynamic drafts сохраняются; команды offline отключены, черновики не применяются автоматически. Sensors показывают unavailable/— и затем valid с прежним ID. [Evidence](evidence/2026-10-10-native-controls-reconnect.json). In-flight mutations, USB hotplug и hardware остаются отдельно.
- [x] Native ambiguous LCD write: proxy пропускает brightness=66 в daemon и разрывает связь после успешного ответа, не доставляя его GUI. Видны error и сохранённый draft; readback=66; proxy count=1 подтверждает отсутствие auto-retry; explicit Apply=65 восстанавливает workflow. [Evidence](evidence/2026-10-10-native-lost-reply.json). Timeout и другие mutation families остаются отдельно.
- [x] Native write timeout: успешный brightness=67 response удерживается proxy при работающем канале чтения; настоящий 10s bridge deadline даёт явное предупреждение о необходимости проверки applied state. Draft сохранён, success не показан, readback=67; proxy count=1, explicit Apply=65 проходит. [Evidence](evidence/2026-10-10-native-write-timeout.json). Проверен общий bridge через LCD patch; это не проверка всех отдельных mutation workflows.
- [x] Внешние Display/Settings updates: polling обновляет state, сохраняет несохранённую тему и оба LCD поля; explicit Load берёт новое состояние, а после Clear сообщает отсутствие темы без потери draft. Команды записи не отправляются. [Browser acceptance](external-control-acceptance.md); native/two-client concurrency отдельно.
- [x] Manual/thermal interrupted browser mutations: reply loss/rejection, A→B→A и reconnect с поздними success/error; новый draft сохранён, submitted snapshot не изменяется, polling/readback восстанавливается, auto-retry отсутствует. [Матрица и ограничения](lighting-interrupted-acceptance.md).
- [x] GUI: все страницы работают через IPC; reconnect/error/pending/applied различаются, preview не выдаётся за аппаратный успех. [Матрица исходников/assertions и native checkpoints](gui-state-acceptance.md); текущий полный browser suite111passed, svelte-check0/0. Expanded accessibility и итоговый интегрированный прогон отдельно.
- [x] Display draft сохраняется при disconnect/reconnect; исчезновение выбранного device не перенаправляет Apply; поздний theme load не затирает новые правки, state старого device отбрасывается. Browser mock acceptance 2026-10-10: `desktop/tests/reconnect.spec.ts`; native/hardware reconnect остаются ниже.
- [x] Cooling late Apply не обновляет другое устройство; late preset не затирает новые правки. Thermal draft Lighting переживает reconnect, а Apply к другому device блокируется. Browser mock: `desktop/tests/control-drafts.spec.ts`, 3 сценария; native reconnect этим не подтверждён. [Checkpoint](implementation-checkpoints-2026-10-14.md#control-drafts-checkpoint-2026-10-10).
- [x] Settings late success/error после смены device не показывается на новом device; Profile edit после reconnect не восстанавливает старую библиотеку; Cleanup scan после изменения draft assets требует rescan. Browser mock: `desktop/tests/settings-library-reconnect.spec.ts`, 4 сценария. Это не отмена принятой daemon-команды и не native reconnect.
- [x] Dynamic RGB отбрасывает late mutation success/error после reconnect и A→B→A; черновик сохраняется. Browser mock: `desktop/tests/dynamic-rgb-reconnect.spec.ts`, 2 сценария. Native reconnect остаётся отдельной проверкой.
- [x] Lighting UI последовательно отправляет manual/thermal/dynamic mutations: Dynamic Apply блокирует остальные Apply, ручной Apply блокирует Dynamic; после success/error блокировка снимается, статус перечитывается, пороги черновика сохраняются. Browser mock: `desktop/tests/lighting-serialization.spec.ts`, 2 сценария; внешние IPC clients и hardware проверяются отдельно.
- [x] Browser keyboard flow: skip-link → main → Settings, единственный aria-current, видимый focus outline, load current → slider → brightness Apply → rotation Apply. `desktop/tests/keyboard.spec.ts`; остальные страницы, native keyboard и полная contrast/labels matrix остаются ниже.
- [x] Browser layer keyboard: перемещение сохраняет focus на перемещённом слое; удаление переводит его к оставшемуся слою, удаление последнего — к добавлению текста. `desktop/tests/layer-keyboard.spec.ts`, focused и полный 32-test browser suite 2026-10-10 прошли; [native regression](evidence/2026-10-10-layer-focus-native-regression.json) также прошёл, но не проверяет native keyboard navigation.
- [x] Browser Cycle keyboard: удаление крайнего/первого показателя сохраняет focus у ближайшего оставшегося source; Tab продолжает редактирование. [Исправление и regression](evidence/2026-10-11-cycle-keyboard.json).
- [x] Expanded LCD editors: accessible names всех5 layer types, profile/font/cleanup; solid-background DOM text contrast и reflow320. Исправлен focus после удаления точки cooling curve. [Coverage и ограничения](accessibility-acceptance.md#раскрытые-редакторы-и-curve-focus-2026-10-11).
- [x] CSS manual soft hyphen: selected-break glyph/width, earlier-break retry, unbroken/explicit/end invisibility и all-align pixels проверены. [Acceptance](text-wrapping-acceptance.md#мягкий-дефис).
- [x] CSS default tab-size8: позиции, near-stop, consecutive tabs, mixed Hebrew, reset/wrap и отсутствие .notdef проверены. [Acceptance](text-wrapping-acceptance.md#табуляция).
- [x] CSS trailing-space alignment: soft wrap vs explicit/end, conditional overflow и NBSP; browser geometry и renderer pixels проверены. [Acceptance](text-wrapping-acceptance.md#выравнивание-концевых-пробелов).
- [x] CSS normal wrapping: CJK punctuation, long-word overflow, NBSP, preserved spaces и bidi continuation совпали с nine-case DOM oracle. [Границы](text-wrapping-acceptance.md).
- [x] CSS text OpenType shaping, paragraph bidi через soft wrap, общие measurement/alignment/render и новые CSS presets: [software acceptance](text-shaping-acceptance.md). Full Windows typography остаётся открытой.
- [x] Font upload cancel с клавиатуры возвращает focus к выбору файла; переход пользователя к другому полю не перехватывается. [Acceptance](accessibility-acceptance.md#отмена-загрузки-шрифта-с-клавиатуры-2026-10-11).
- [x] Native keyboard: Xvfb/Openbox + реальные X11 key events, 132 trusted steps; skip-link и основные workflows пяти страниц, computed outline, IPC readback. [Результаты](native-keyboard-acceptance.md). Последующие Alt+Tab, AT-SPI и numeric checks приведены ниже; speech/zoom и остальные expanded states отдельно.
- [x] Native Dynamic RGB invalid/recovery: inverted thresholds не меняют daemon config; оба INVALID_ENTRY, доступный текст ошибки и исправление подтверждены AT-SPI. 400 keyboard steps/12 variants; speech/live announcements отдельно. [Evidence](evidence/2026-10-11-native-dynamic-invalid.json).
- [x] Browser text/status/error contrast: main-текст пяти начальных страниц, Settings/Display RPC errors, curve validation/recovery, Cooling/Lighting/Sensors success; сохранены expanded editor checks.11focused tests, svelte-check0/0. [Границы](accessibility-acceptance.md#textstatuserror-contrast--2026-10-11); input values/popups/disabled/forced-colors/native отдельно.
- [x] Cooling numeric validation согласована с Rust: integer PWM/RPM/fallback, mode-dependent °C, −50…150°C/гистерезис0…10°C; invalid drafts не отправляют RPC, correction работает. Browser13passed, native400steps/12variants. [Контракт](accessibility-acceptance.md#числовой-контракт-cooling--2026-10-11).
- [x] Connection/persistence имеют стабильные polite atomic live regions; error — отдельный alert. Browser подтвердил node identity и отсутствие mutations на неизменном polling; native AT-SPI connection attributes/text прошли. [Evidence](evidence/2026-10-11-live-regions.json). Spoken announcements ещё не проверены.
- [x] Native accessibility bus доставляет polite pending/saved и assertive Dynamic RGB validation events: private listener,24events,400keyboard steps/12snapshots. [Evidence](evidence/2026-10-11-native-live-events.json). Это не speech acceptance; connection reconnect принят отдельным checkpoint ниже.
- [x] Native connection offline/reconnect публикует polite children-added events; Apply отключается/восстанавливается, unchanged polling не меняет live region. Исправлен текстовый узел для WebKitGTK.402keyboard steps/37events,13browser,612Rust. [Evidence](evidence/2026-10-11-native-connection-events.json). Speech остаётся отдельно.
- [x] Native WebKit zoom 200%/400% на окне1280×960: viewport640/320CSSpx, все пять страниц без горизонтального overflow, по403keyboard steps/12AT-SPI variants с IPC readback и упорядоченным offline→online event. [Evidence](evidence/2026-10-11-native-zoom.json). Не закрывает OS scaling, все expanded states, отдельную обрезку текста или ширину212CSSpx.
- [x] Native GTK image chooser: cancel/focus/unchanged draft, выбор и повторный выбор PNG, draft-only import → explicit Apply → JPEG preview, совпадение normalized PNG bytes.434keyboard steps/12AT-SPI variants. [Evidence](evidence/2026-10-11-native-image-chooser.json). Font/scaled dialogs отдельно; success focus проверен ниже.
- [x] Image import focus: success/error → keyboard continuation; новый focus и другая страница не затрагиваются;32слой → focusable heading.13browserpassed, native success/reselect focus подтверждён после реального GTK picker. [Evidence](evidence/2026-10-11-image-import-focus.json). Native error/limit cases отдельно.
- [ ] GUI: оставшаяся keyboard navigation, labels/focus/contrast, validation, unsaved edits и readable errors; UI tests на отдельном display.
- [x] Native profiles: screen save/rename/load/reset/delete; pump/fan save/load в независимые slots; отсутствие Apply от операций библиотеки, полное содержимое и revision после restart. [Evidence](evidence/2026-10-10-native-profiles.json).
- [x] Native profile conflict: external IPC rename → stale GUI edit rejected → оба черновика сохранены → refresh → retry; active LCD и external library не затёрты. [Evidence](evidence/2026-10-10-native-profile-conflict.json).
- [x] Native profile error paths: missing image save/Apply отклоняется без изменения library/revision/active theme; неизвестный sensor виден после DOM load, Apply отклоняется, рабочая cooling policy сохраняется, допустимый профиль снова применяется. [Evidence](evidence/2026-10-10-native-profile-missing.json). Потеря уже активного источника — отдельный fallback scenario.
- [x] Native active pump sensor: исчезновение fixture:cpu при сохранённой curve включает configured fallback; GUI показывает недоступность и сохраняет draft/ID. Возврат того же ID восстанавливает curve без Apply черновика, LCD bindings не меняются. [Evidence](evidence/2026-10-10-native-active-sensor.json). Это pump missing-source, не все stale/thermal/fan случаи.
- [x] Native profile save с потерей успешного ответа: revision +1 и ровно один запрос, сохранён submitted snapshot, последующие editor edits и активная LCD-тема не затронуты; refresh/readback и explicit load/save восстанавливают workflow. [Evidence](evidence/2026-10-10-native-profile-lost-reply.json). Rename/delete/reset и обрыв до commit отдельно не инъецировались.
- [x] Browser profile mutations: save/rename/delete/reset с потерянным ответом после commit; поздние success/error после reconnect; нет auto-retry, сохраняется editor draft, readback обязателен. [Evidence](evidence/2026-10-10-profiles-interrupted.json). Native regression также прошёл.
- [x] Browser profile rejection до commit: save/rename/delete/reset сохраняют library/revision и draft; pump save → fan save → поздний pump success/error не меняет fan state и ничего не применяет. [Evidence](evidence/2026-10-10-profile-precommit-group.json). Это fixture rejection, не физический обрыв транспорта до persistence.
- [x] Native profile rename/reset/delete: потеря успешного ответа, ровно один commit, draft сохранён, readback без retry; save обрывается до передачи daemon и не меняет library/revision. [Evidence](evidence/2026-10-10-native-profile-mutations.json). Это не crash внутри persistence.
- [x] Profile persistence: старый/committed файл при ошибках на границах write/fsync/rename, reopen, stale revision и explicit recovery; существующие CRUD/concurrency/schema/asset тесты сопоставлены. [Acceptance](profile-persistence-acceptance.md). Не означает power-loss acceptance.
- [x] Native pump RGB write failure: ошибка видна, binding сохранён; LCD отправляет новые JPEG, fan PWM61 применяется; снятие fault восстанавливает RGB без Apply, fan55 и restart проверены. [Evidence](evidence/2026-10-11-native-rgb-transport.json). LCD/PWM failures и direct-color остаются отдельными сценариями.
- [x] Независимость активных screen/cooling/lighting consumers при одиночных ошибках записи: native RGB/JPEG/fan PWM с сохранением intent и recovery; component direct/H264/partial batch. [Матрица и границы](transport-consumer-acceptance.md). Это software acceptance, не физический USB resync.
- [x] Native image cleanup: dry-run/delete удаляет единственный orphan; profile-only/offline-device-only и active theme image сохранены; оба защищённых assets рендерятся после restart. [Evidence](evidence/2026-10-10-native-image-cleanup.json).
- [x] Native cleanup: offline settings повреждены между dry-run и Delete → ошибка и сброс stale preview, все seeded bytes сохранены; восстановление settings → fresh scan → explicit delete. [Evidence](evidence/2026-10-10-native-cleanup-corrupt.json).
- [x] Native font cleanup: profile-only/offline-device-only fonts защищены; orphan удалён; bytes и рендер обеих тем после restart сохранены. [Evidence](evidence/2026-10-10-native-font-retention.json).
- [x] Native image/font draft-only и session-only retention: отдельные категории защиты после удаления профиля и смены черновика. [Evidence](evidence/2026-10-10-native-draft-session-retention.json).
- [x] Native corrupt profile JSON: image Delete/font scan отклонены без изменения файлов; restore → fresh scan → recovery. [Evidence](evidence/2026-10-10-native-cleanup-profiles.json).
- [x] Cleanup: native image/font partial unlink failure → fresh scan → retry; successful subset и failing key видны, файлы и active theme проверены. [Evidence](evidence/2026-10-10-native-cleanup-partial.json). [Сводная matrix](cleanup-acceptance.md); backend/browser coverage уже есть.
- [x] Интеграционный checkpoint 2026-10-11:599 Rust,110 browser,26 contracts,release build,native smoke и132 keyboard steps на неизменных source hashes. [Evidence](evidence/2026-10-11-integrated-software.json). Финальная приёмка после закрытия остальных gaps остаётся ниже/в плане.
- [x] Документация: [единый quickstart/troubleshooting](../quickstart.md), ссылки на schema и пример темы; CLI help и пример проверены offline. Закрытые старые планы удалены, три незавершённых сохранены. Установка на реальном устройстве отдельно.

- [x] hwmon fan RPM collector: direct RPM, stopped0/fault/disabled/malformed, renumber/missing/recovery/stale, duplicate identity и overflow проверены fixtures. [Evidence](evidence/2026-10-10-hwmon-fan-rpm.json). Это не CPU-fan mapping и не физическая проверка.

- [x] Missing/stale consumers: сопоставлены cooling/thermal/dynamic RGB/LCD tests; fan fallback не подменяет binding чужим ID и восстанавливает прежний; LCD cache→JPEG различает unavailable и настоящий zero, восстанавливается без Apply. [Матрица и границы](sensor-consumer-acceptance.md). Native missing/same-ID и aged-stale/hot→cool также закрыты (см. ту же матрицу); software transport independence [проверена](transport-consumer-acceptance.md).

- [x] Opt-in ASUS WMI CPU voltage: exact rail, mV→V, stable semantic binding, duplicate ambiguity, source selection/missing recovery, invalid/zero/stale проверены fixtures. [Evidence](evidence/2026-10-11-asus-wmi-voltage.json). Cache→IPC→LCD preview, stale и same-binding recovery [также проверены](evidence/2026-10-11-asus-wmi-preview.json). Наличие/accuracy/BIOS latency требуют аппаратной проверки.

- [x] CSS glyph fallback: primary→DejaVu Sans→Noto Sans TC с целыми grapheme clusters, canonical composition, shared measurement/raster; browser advances, fallback outlines, wrapping и legacy regression проверены. [Acceptance и ограничения](text-shaping-acceptance.md#fallback-отсутствующих-glyphs--2026-10-11). Windows font resolver/native/LCD отдельно.

- [x] Native renderer follow-up после SHY/fallback: WebKitGTK→IPC→MockBackend, CJK/canonical-accent preview, mixed text Apply/profile/restart и GUI-closed frames. [Source hashes и полный readback](evidence/2026-10-11-native-text-fallback.json). Native AT/zoom и hardware остаются отдельно.

- [x] Native window round trip: реальный Alt+Tab в отдельное окно и обратно сохраняет Settings brightness focus; trusted blur/focus, X11 active-window IDs и дальнейший keyboard Apply проверены. [Evidence](evidence/2026-10-11-native-window-focus.json). AT/zoom/dialogs/Wayland отдельно.

- [x] Native AT-SPI: navigation/Settings names и roles, enabled/focused brightness slider через private accessibility bus проверены; последующие132 keyboard steps прошли. [Evidence](evidence/2026-10-11-native-atspi.json). Speech/live regions/другие страницы и expanded controls отдельно.

- [x] Native AT-SPI main controls пяти страниц: document/navigation, Apply names/roles, focused/enabled target и последующие keyboard→IPC действия проверены. [Полные trees](evidence/2026-10-11-native-atspi-pages.json).
- [x] Fixed PWM numeric semantics: explicit spinbutton/ARIA bridge экспортирует Value79/min10/max100 после native keyboard edit; PWM/RPM/channel/invalid transitions проверены browser. [Исправление и границы](accessibility-acceptance.md#fixed-pwmrpm-numeric-semantics--2026-10-11). Native RPM, прочие numeric controls и screen-reader speech остаются в expanded acceptance.

- [x] Cooling curve numeric semantics: temperature/PWM/RPM/hysteresis/fallback bounds/value/invalid; browser invalid/recovery/no-write и native fan curve AT-SPI Value31 после редактирования,147 trusted steps. [Evidence](evidence/2026-10-11-curve-numeric-accessibility.json). Native pump/RPM и speech отдельно.

- [x] Theme identity replay: другой serial на прежнем USB-пути остаётся без темы/JPEG; исходный serial на новом пути восстанавливает композицию и binding с полным ожидаемым JPEG batch; обе сохранённые темы изолированы. [Сценарий и границы](persistence-acceptance.md#тема-следует-за-serial-а-не-usb-путём--2026-10-11). MockBackend, не физический reconnect.

- [x] Periodic render responsiveness: LCD State/Clear/manual image во время задержанного render future; late success/error отбрасываются по generation, shutdown joins, старый JPEG не возвращается. [Сценарии и ограничения](theme-preparation-order.md#команды-во-время-периодического-render--2026-10-11). Это не полный IPC/cooling/RGB slow-encode checkpoint.

- [x] First-frame preparation до LCD queue: State/Clear отвечают при занятом blocking worker, поздний prepared Apply отклонён без JPEG; IPC permit покрывает asset load и первый render. [Контракт и ограничения](theme-preparation-order.md#первый-кадр-до-очереди-lcd-команд--2026-10-11). Доступность одного JSON-RPC stream и совместный cooling/RGB сценарий отдельно.

- [x] Один Unix IPC stream: задержанный Apply/preview не блокирует State/Clear/manual image/PWM/RGB; bounded queue/busy, superseded/recovery, fresh mock writes и no stale JPEG проверены. [Admission и границы](theme-preparation-order.md#applypreview-не-блокируют-один-ipc-stream--2026-10-11). Задержка asset I/O, не heavy CPU benchmark.

- [x] Native pump fixed/curve RPM: AT-SPI spinbutton/Value ranges, focused/enabled и current2201/temperature31;203 trusted keyboard steps, Apply и daemon requested/applied, отсутствие записи при draft edit. [Evidence и границы](native-keyboard-acceptance.md#pump-rpm-native-acceptance--2026-10-11). Speech/прочие numeric/zoom отдельно.

- [x] Lighting numeric: thermal finite/range validation до RPC; thresholds/speeds spinbutton/Value/invalid; browser invalid/recovery и native thermal keyboard/AT-SPI/Apply/readback. [Evidence и ограничения](accessibility-acceptance.md#lighting-numeric-semantics--2026-10-11). Native Dynamic RGB probe и speech остаются открытыми.

- [x] Native Dynamic RGB numeric: Celsius/RPM threshold bounds/current, speed bounds/current, focused/enabled; draft не пишет, Apply/readback и disable работают.320 trusted steps,11AT-SPI probes. [Evidence и границы](native-keyboard-acceptance.md#dynamic-rgb-native-numeric-acceptance--2026-10-11). Speech и native invalid announcements отдельно.

## Подготовка аппаратного прогона

- [ ] Записать firmware/SKU/device identity и git state; убедиться, что HID имеет одного owner.
- [ ] Сохранить рабочую конфигурацию; отдельно установить pump duty/source для восстановления.
- [ ] Установить, подключены ли fan к LCDV2; если к материнке — fan hardware сценарии отмечать unavailable, не pass.
- [ ] По завершении вернуть LocalUI; восстановить разрешённые cooling/RGB settings и проверить завершение тестового daemon.

## LCD и realtime sensors

- [ ] Intel Windows baseline: сохранить hwinfo-report.xml, hwinfo-sensors.json, выбранный GPULoad mapping и timestamped load на render/compute/video workload; сопоставить физический adapter. [Source boundary и пути](intel-engine-utilization.md#установленная-граница-sdk-2026-10-11). Один sensor label не доказывает общую формулу.


- [ ] xe PMU: support/GuC/permissions, известная render/compute/video нагрузка,
  hotplug/CPU migration/sleep, idle power/latency и отсутствие пауз LCD.


- [ ] Intel VRAM на discrete i915/xe и iGPU: права render node/query, total/used,
  redacted accounting, multi-GPU mapping, renumber/hotplug, latency без LCD stalls.
  Нулевой расход сейчас unavailable; проверка не должна менять capabilities.


- [ ] На поддерживаемой ASUS-плате сверить asusec in0 с GUI/LCD в покое и под нагрузкой; проверить наличие канала и latency. Отсутствие канала — unavailable, не ноль и не VID. Не загружать драйвер и не менять ACPI mutex settings автоматически.

- [ ] JPEG с EXIF rotation/mirror после повторного импорта отображается как ожидается; orientation1…8, portrait/landscape, fit/crop, сохранение и restart. Старые нормализованные assets требуют оригинального файла.
- [ ] Собственное изображение появляется без искажений и сохраняется >70 секунд; заменить другим изображением.
- [ ] Solid/gradient backgrounds: направления, оба цвета и Flip видимы на LCD; radial центр/края/углы, overlays и profile/restart соответствуют preview.
- [ ] Fit/crop/contain/stretch корректны для portrait/landscape; transparency/opacity не дают чёрных артефактов; image shadow следует прозрачному контуру и корректно обрезается у края LCD. Watermark center placement 0/50/100 соответствует preview, в том числе при частичном выходе за экран.
- [ ] CPU/GPU/coolant/RPM values на LCD соответствуют IPC и обновляются при изменении нагрузки.
- [ ] Потеря датчика отображается unavailable, восстановление возвращает правильный binding; нет fake zero.
- [ ] Часы/дата/AM-PM обновляются, включая полночь/полдень и смену даты; отдельные цвета сохраняются. Unicode readable, все presets/цвета/размеры/раскладки соответствуют preview.
- [ ] Закрыть GUI: картинка, часы, sensors и cooling продолжают работать.
- [ ] Brightness и rotation 0/90/180/270 дают видимый результат и сохраняют соседние настройки.
- [ ] Быстро применить несколько тем/изображений, затем clear: нет старых кадров, final LocalUI подтверждён визуально.
- [ ] 30 минут realtime theme + sensors + RGB + cooling: без пауз/артефактов, bounded CPU/RSS/queue, записать FPS/latency/sensor age/errors.

## Управление и lifecycle

- [ ] Pump presets/curve/source изменяют измеряемые RPM; fallback и manual override работают с известным restore.
- [ ] Fan presets/curve проверены на подключённых к LCDV2 fan; иначе явный unavailable.
- [ ] Все доступные RGB modes/sync/LED counts/thermal transitions видимы; после alert/grace возвращается base по всей доступной зоне.
- [ ] Daemon restart восстанавливает выбранную тему/settings и свежие readings; ошибочное replay не показывает applied.
- [ ] USB reconnect: stable identity, восстановление settings, старые frames/actors не возвращаются. Ранее отложен пользователем.
- [ ] Sleep/wake: orderly stop/resume, обновление времени/readings и controls; без второй HID session.
- [ ] Cold boot/autostart: GUI и daemon стартуют согласно настройке; рабочий screen после готовности daemon.
- [ ] Изолированный transport failure/recovery по согласованному безопасному сценарию; без firmware flashing/fault injection на единственном AIO.

Firmware update, Windows-compatible archive import, full video editor/capture
и полный набор фирменных effects относятся к расширенному parity roadmap;
не считать их выполненными успешной приёмкой базового набора.

## Optional NVIDIA voltage: аппаратная применимость

- [ ] В явно выбранной X-сессии сверить UUID и GPUCurrentCoreVoltage с LCD/GUI.
- [ ] Проверить unsupported GPU/отсутствие nvidia-settings, запрет Xauthority и
  завершение X-сессии: unavailable без потери остальных NVIDIA readings.
- [ ] Проверить две GPU и перестановку enumeration; voltage остаётся у того же UUID.
- [ ] Отдельно записать доступность XWayland/Wayland/headless; не считать
  отсутствие NV-CONTROL подтверждением полного NVIDIA voltage parity.

## Optional ASUS WMI voltage: аппаратная применимость

- [ ] На поддержанной плате с подходящим BIOS и уже загруженным драйвером подтвердить точную метку CPU Core Voltage, единицу и соответствие reference reading.
- [ ] Явно включить дополнительный опрос только после проверки BIOS limitations; проверить latency, стабильность fan/sensors и отсутствие пауз LCD.
- [ ] Если CPU Core Voltage продублирован SIO/EC, подтвердить invalid; не выбирать произвольный rail и не подменять CPU SOC/VID.
- [ ] Проверить выключение дополнительного voltage collector и missing/recovery binding; существующий fan polling остаётся отдельным поведением.


- [x] GTK PNG chooser совместно с WebKit zoom400%/320CSSpx: cancel/reselect/focus/draft/Apply/preview;434keyboard steps/12AT-SPI variants/41events. [Evidence](evidence/2026-10-11-image-chooser-webkit-zoom.json). OS font/dialog scaling, font chooser, native error/limit и speech остаются отдельно.

- [x] Manager sleep/wake foundation: joined HID teardown, paused discovery, durable theme replay, fresh runtime ID, no diagnostic restart, duplicate events и shutdown while suspended. [Границы и logind acceptance](suspend-resume-acceptance.md). Сам foundation не является аппаратной приёмкой.

- [x] Production logind observer: ordered sleep/wake, inhibitor release/reacquire, owner-loss resync, blocked HID deadline, supervisor shutdown и host sensor invalidation.622Rust tests/92targets; [evidence](evidence/2026-10-11-logind-integration.json). Последующие software проверки перечислены ниже; hardware отдельно.

- [x] Logind failure acceptance: полный перезапуск private bus, отсутствие сервиса при старте и recovery, preemption зависшего health property query сигналом сна.10logind tests прошли; [границы](suspend-resume-acceptance.md). Production код в этом checkpoint не менялся; combined проверен следующим checkpoint, hardware остаётся.

- [x] Combined sleep/wake с synthetic CPU: cooling fallback100/100 → hot90/80 → cool45/50, thermal alert/base обеих зон, полные stale/hot/cool JPEG batches и сохранение base settings. [Границы](suspend-resume-acceptance.md). Rate/counter baselines проверены следующим checkpoint; hardware остаётся.

- [x] Sleep/wake сбрасывает CPU/RAPL/network/i915/xe baselines; прерванный async collection отклонён, новые rates требуют свежей пары отсчётов.631Rust tests/93targets, fmt/strict Clippy/docs; [evidence](evidence/2026-10-11-sleep-counter-baselines.json). Физический сон и аппаратные readings не проверялись.

- [x] Native GTK font chooser при100%/X11: cancel/select/reselect, focus, explicit font selection/Apply и JPEG preview через MockBackend; [evidence](evidence/2026-10-11-native-font-chooser.json). Zoom400 для font chooser также проверен [отдельно](evidence/2026-10-11-font-chooser-webkit-zoom.json); OS scaling, error/limit states и speech остаются.

- [x] Native font invalid/oversize → error + focus → valid retry/apply, неизменная тема при отказе, assertive events и expanded reflow320 при WebKit400%; [evidence](evidence/2026-10-11-native-font-errors.json). Остальные editor errors/limits и OS scaling отдельно.

- [x] Общий checkpoint текущей реализации после sleep/font:631Rust/119browser/26contracts, fmt/Clippy/docs, check/build/release и оба native harness на329 неизменных файлах. [Evidence](evidence/2026-10-11-integrated-font-sleep.json). Открытые требования этим не закрыты.

- [x] NCT6779/6791/6792/6793/6795/6796/6797/6798/6799 CPUVC in0: mV→V, stable identity, CPU source selection, malformed/disabled/stale/missing/recovery и cache→IPC→LCD preview. [Evidence](evidence/2026-10-11-nct-cpu-voltage.json). Назначение входа подтверждено Linux source; разводка/точность каждой платы остаётся hardware acceptance.

- [x] X11 OS text DPI96/192 и GTK PNG/TTF chooser: effective settings, glyph19→37px, доступные path/Open, cancel/select/reselect, font errors/retry; совместно с WebKit zoom200% — viewport320 без overflow на проверенных states. [Evidence](evidence/2026-10-11-native-os-text-scaling.json). Wayland/fractional/другие fonts/themes/speech и остальные expanded states отдельно.

- [x] GPU query lifecycle: NVIDIA CSV/NV-CONTROL и общий Intel query helper сохраняют Child при cancellation, ограничивают reap100ms и не допускают overlap. Voltage UUID isolation/cap32/disappearance/recovery проверены fixtures. [Evidence](evidence/2026-10-11-nvidia-bounded-reaping.json). Фактический uninterruptible kernel I/O не воспроизводился.

- [x] Nouveau GPU core voltage: только in0, mV→V, stable identity/source selection,
  invalid/stale/disappearance/recovery и cache→IPC→LCD preview проверены fixtures.
  [Evidence](evidence/2026-10-11-nouveau-voltage.json).
- [ ] На доступном nouveau GPU проверить наличие, driver-reported значение и
  latency канала; сопоставить с driver source. Не считать это доказательством
  аналогового измерения или proprietary NVIDIA headless coverage.

- [x] M4 preparation: real GIF/MP4 decode/encode, contain/cover, non-square pixels,
  frame delays, bounded rejection и CLI manifest/IPC contract проверены.
  [Evidence](evidence/2026-10-11-media-preparation.json).
- [ ] M4 media lifecycle: native publication faults/accessibility, полный
  Screen reset/состав коллекций, расширенная fault/performance и совместная lifecycle приёмка,
  затем физическая проверка на LCD. Bounded import/loop/replay уже реализованы.

- [x] Prepared clip library: import/list/load через IPC без устройства,
  атомарная публикация, повторное чтение после restart, corrupt/version/path
  rejection и concurrent quota проверены software fixtures.
- [x] Сохранённый media choice в device settings и LCD actor: repeat/restart,
  missing asset/recovery, Clear снимает autoplay, stale preparation не заменяет новый source.
- [ ] Полный Screen reset (коллекции, карточки, defaults и контексты выбора реализованы);
  совместная media/logind fault и hardware приёмка.

- [x] CLI prepared library: offline import/list → restart read → saved-key play
  → Clear через private IPC/MockBackend; весь H.264 packet batch и LocalUI
  write совпадают с ожидаемыми. Invalid local input не подключается к daemon.

- [x] Prepared media manager sleep/wake: joined stop, no writes во сне,
  old-ID rejection, fresh-ID full-frame replay, Clear и restart без autoplay.
- [x] Device schema: media key roundtrip, legacy None, theme/media conflict
  и invalid key не заменяют валидный TOML; соседний fan PWM сохранён.

- [x] GUI prepared library: список, сохранённый выбор, select/stop→LocalUI;
  draft темы сохраняется, ошибки допускают retry, lost device и поздний ответ
  предыдущего устройства не перенаправляют команду. Browser/bridge checks:
  [evidence](evidence/2026-10-11-media-library-gui.json).
- [ ] Полный keyboard/focus/reflow/AT-SPI и физический LCD для media panel;
  native file chooser и импорт проверены отдельно ниже.

- [x] Native preparation backend: real GIF/MP4 → validated H.264, contain/cover
  pixels, input bounds и cancellation с reap child проверены программно.
  [Evidence](evidence/2026-10-11-native-media-preparation.json).
- [x] Подключить native preparation к GUI: bounded upload, один job/admission,
  phase progress/cancel, daemon publication без автоматического Apply;
  ordered upload/cancel/unmount/retry и shutdown проверены software fixtures.
  [Evidence](evidence/2026-10-11-managed-media-import.json).
- [x] Native WebKit→FFmpeg→private daemon импорт: GTK chooser cancel/MP4/GIF,
  malformed input, contain/cover decode, progress, worker cancel, restart library
  и обычный close во время конвертации. [Evidence](evidence/2026-10-11-native-media-import.json).
- [ ] Native disconnect/ambiguous publication и close во время publication;
  полный keyboard/focus/reflow/AT-SPI media panel, затем аппаратный playback.
  Forced exit/crash/OS logout приложения отдельно.

## Миниатюра выбранного ролика — 2026-10-12

- [x] Первый кадр из сохранённого H.264:144×144, contain/cover, bounded decode,
  cancellation/reap, waiter abort, stale response discard и URL cleanup.
- [x] Native MP4/GIF preview и новый preview после daemon restart без Apply.
  [Evidence](evidence/2026-10-12-media-thumbnail.json):128browser tests;
  первый native run имел GTK/AT-SPI readiness failure, повтор прошёл.
- [x] Контексты выбора, protected defaults и thumbnails реализованы (checkpoints ниже).
- [ ] Native chooser flakiness требует отдельной стабилизации, не скрывать retry.

## Media deletion — основа хранения

- [x] Store acquire/delete сериализованы, несколько pins защищают файл до
  последнего drop; неизвестные durable references запрещают unlink; quota
  освобождается, соседние records и symlinks не удаляются.
- [x] Pin в LCD preparation/actor/replay и pending settings до durable commit;
  strict offline settings scan и delete RPC/CLI/GUI проверены. Clear до commit
  не позволяет удалить старую сохранённую ссылку; ошибка записи сохраняет pin.
- [x] CLI active/sleep rejection → Clear/restart → delete; GUI confirmation,
  lost reply/readback без retry, focus; native delete/reimport без смены LCD.
  [Evidence](evidence/2026-10-12-media-delete.json):665Rust/131browser.
- [x] Media slots: strict scan профилей и pin до durable publication, same-group
  slot validation, save/delete race, missing asset rejection и restart.
- [x] GUI/CLI save/load/explicit Apply, rename/reset/delete; native media slot
  workflow прошёл. [Evidence](evidence/2026-10-12-media-slots.json).
- [ ] Native deletion transport faults, полная доступность и hardware остаются.


## Карточки видео — 2026-10-12

- [x] Pagination по8, page retention при metadata refresh, radio selection без Apply.
- [x] Последовательная очередь, отмена queued requests до dispatch, удержание
  active до native completion, stale result discard и URL cleanup.
- [x] Native WebKit: две реальные144px FFmpeg миниатюры, выбор обоих роликов
  без playback и возврат к selected preview; profile/delete/restart/cancel/close.
  [Evidence](evidence/2026-10-12-media-gallery.json):135browser tests.
- [ ] Native keyboard/screen-reader и expanded scaling для новых cards/pagination.
- [x] Контексты выбора image/video background и watermark, defaults; checkpoints ниже.
- [ ] Hardware media acceptance; GUI composition и long-video import/runtime приняты программно ниже.
  Единая глобальная gallery не является требованием Windows baseline.


## Сохранённые изображения — 2026-10-12

- [x] List/restart, ограниченный preview, symlink/path rejection; просмотр не
  закрепляет assets и не меняет cleanup eligibility.
- [x] Pagination, corrupt-image isolation, late-result discard/URL cleanup,
  добавление в черновик и только явный Apply;137browser tests.
- [x] Native import→gallery→draft с настоящими IPC/image decoder без изменения
  LCD source. [Evidence](evidence/2026-10-12-image-gallery.json).
- [x] Контексты выбора и defaults; native image delete принят в builtin-video checkpoint ниже.
- [ ] Полная native keyboard/speech/scaling и физический image/theme output.


## Имена изображений — 2026-10-12

- [x] Named import и legacy без metadata; rename/revision conflict, concurrent
  edit, PNG/reference preservation, unknown-version rejection и sidecar cleanup.
- [x] GUI rename и lost-reply readback без autoretry/Apply,139browser tests.
- [x] Native named import→rename→restart;[evidence](evidence/2026-10-12-image-names.json).
- [ ] Полная keyboard/speech/scaling приёмка формы rename; GTK chooser flakiness.


## Defaults/reference — 2026-10-12

- [x] Состав defaults, three-gallery contexts, запрет delete/editor, Screen reset
  и rollback прослежены по2.1.28.0; ресурсы измерены ffprobe, hashes сохранены.
  [Исследование и полный acceptance](gallery-defaults.md).
- [x] GUI file-backed import принимает синтетическую штатную длительность24.416667s/586frames,
  VP9-in-MP4→H.264 без обрезания, file owner и один AU/RPC; native mock loop/restart
  и decoder586frames проверены. Это не физическая приёмка и не импорт proprietary assets.
- [ ] Builtin bootstrap/protection/restore без сброса пользовательских profiles.
- [ ] Отдельный полный Screen reset50%/0°/8slots и failure recovery.


## Persistent media v2 — основа

- [x] Indexed file writer/reader, bounded header/AU, no-clobber и drop staging;
  corrupt index/payload/truncation/symlink rejection.
- [x] Synthetic VP9-in-MP424.416667s/586frames→H264→file→decoder586frames,
  encoded payload>1MiB. [Evidence](evidence/2026-10-12-media-file.json).
- [x] Store v1/v2 list/open/delete, shared quota/pins, profile Save protection;
  [evidence](evidence/2026-10-12-media-store-v2.json).
- [x] Reader worker: bounded prefetch, joined/cancelled stop, panic/repeated stop,
  outstanding-permit race, legacy/v2 loop586 и file/pending-save pins;
  [evidence](evidence/2026-10-12-media-reader.json).
- [x] Reader→LCD actor, complete586frames/loop, replace/clear и file pins;
  CLI/private IPC profile/restart/missing/recovery/sleep/wake/clear/delete через v2.
  [Evidence](evidence/2026-10-12-media-actor.json).
- [x] Chunked IPC publication: bounded AU, order/retry/concurrent Finish,
  cancel/quota, lost Finish reply/restart receipt, idle expiry и crash staging cleanup.
  [Evidence](evidence/2026-10-12-media-upload.json).
- [x] RPC первого AU v1/v2 и подключение native thumbnail без загрузки всего v2
  payload. RPC regressions пройдены; это не native UI acceptance.
- [x] Native v2 thumbnail: 586-frame asset → 144px cyan preview → delete;
  selection не запускает playback и не оставляет deletion pin.
  [Evidence](evidence/2026-10-12-media-first-frame.json).
- [x] File preparation: synthetic VP9 586frames/24,416667s → bounded H.264 file
  → AU read → decoder586frames, byte budget/cancellation/cleanup.
  Это отдельная подготовка, ещё не GUI import→publication.
  [Checkpoint и native повтор](evidence/2026-10-12-media-file-preparation.json).
- [x] Native/GUI long-video import через v2, transferring cancellation и
  receipt recovery через Status/Cancel. [Checkpoint](evidence/2026-10-12-native-long-import.json).
- [x] Атомарная version2 theme identity: video key + layers, settings/profile
  round-trip, missing media refusal и delete protection.
- [x] Composition backend Apply/replay, restart/sleep/wake, missing-media recovery
  и Clear с real codec/private IPC/MockBackend; GUI и hardware отдельно.
- [x] Backend preview первого видеокадра с текущими readings/cycle elapsed, bounded
  decode и cleanup без LCD writes. [Evidence](evidence/2026-10-12-video-composition-preview.json).
- [x] GUI video background → preview RPC → screen slot → явный Apply и возврат
  к static background проверены browser tests; layers/gradient сохраняются,
  missing video вызывает явную ошибку.
- [x] Protected image/video defaults (checkpoints ниже).
- [ ] Полная native accessibility композиции.
  [Video composition: готовая основа и оставшаяся приёмка](video-composition.md).
- [ ] Дополнительная fault acceptance: медленное чтение/first-frame cancellation,
  повреждение файла во время actor playback и native/GUI publication disconnect/reply loss.
  Daemon IPC lost Finish reply проверен отдельно; UI outcome ещё не принят.
- [x] Native GUI composition: preview pixels, Apply/readback video+clock, Clear
  и delete после persistence. [Evidence](evidence/2026-10-12-video-background-gui.json).
- [ ] Расширенные native publication fault/accessibility проверки;
  физическая приёмка нового runtime. Native standalone long-video import/loop/restart уже проверены.

## Сверка после composition faults и image delete — 2026-10-12

- [x] Codec faults и Stop с заполненной очередью: bounded error, joined cleanup,
  release pin; [evidence](evidence/2026-10-12-composition-codec-faults.json).
- [x] Early IPC EOF и потерянный Apply response с отличимой новой темой;
  [evidence](evidence/2026-10-12-composition-ipc-faults.json). Это не cancellation
  посреди codec preparation.
- [x] Начальное release измерение: 720frames/24fps/30s с sensor+clock;
  [границы измерения](evidence/2026-10-12-composition-paced-release.json).
  Сложная длительная композиция с actor/HID ещё не квалифицирована.
- [x] Точечное image delete: revision/draft/live lease/offline settings/profile
  protection, fail-closed при corrupt references, symlink refusal; backend tests.
  Browser confirm/cancel, lost-reply readback, close/reopen, focus и draft guard:
  целевой повтор 7 tests прошёл на текущем frontend.
- [ ] Native image delete и полная keyboard/speech/scaling приёмка.
  [Сверка и ограничения](evidence/2026-10-12-plan-audit-after-image-delete.json).

## Встроенные image defaults — 2026-10-12

- [x] Собственные mark/background PNG, стабильные versioned keys и immutable
  embedded catalog без bootstrap-записи. List, thumbnails и theme load доступны
  после restart; disk shadow не меняет artwork/metadata.
- [x] Backend rename/delete refusal, cleanup safety, обычный user import/rename/delete;
  IPC protections и browser Add/disabled mutations проверены.
- [x] Native thumbnails и disabled actions, пользовательский image delete + readback
  и focus пройдены до sequence11. Полный native harness НЕ прошёл: последний
  chooser_slow завершился ошибкой AT-SPI path/Open; отдельного acceptance.json нет.
- [ ] Устранить/квалифицировать последний native chooser failure; speech/scaling.
- [x] Встроенное видео и контексты выбора галерей (последующие checkpoints).
- [ ] Полный Screen reset/recovery.
  [Evidence и точные границы](evidence/2026-10-12-builtin-images.json).

## Встроенное видео и итог native media — 2026-10-12

- [x] Собственный builtin video, bounded AU streaming, полный decode576frames,
  анимация, защита удаления/публикации/disk shadow и сохранение user quota.
- [x] GUI playback/background selection и delete guard; native preview/Apply/Clear.
- [x] Полный native media workflow повторён после ожидания Stop и обработки GLib
  событий: defaults, image deletion, import/long loop/restart/composition/close.
  Это закрывает базовую native image-delete приёмку из предыдущих checkpoints.
- [x] Software checkpoint:721Rust/8focused browser, strict Clippy, fmt/docs,
  frontend check/build. [Evidence](evidence/2026-10-12-builtin-video.json).
- [ ] Full Screen reset/recovery; расширенная accessibility
  и native fault matrix; hardware позже.

## Контексты галереи и статический фон — 2026-10-12

- [x] Выбор image/background/watermark; фильтрация defaults по назначению.
- [x] Статический фон под overlays, четыре fit, прозрачность; watermark сверху.
- [x] Переключение image/video/solid сохраняет слои; выбор не применяет тему.
- [x] Background references защищены live lease, offline settings и profiles;
  delete/cleanup учитывают их наряду с image layers.
- [x] Schema round-trip, renderer/missing asset и cross-language contracts.
- [ ] Full Screen reset, recovery и расширенная
  accessibility/lifecycle/hardware qualification.

Точные результаты текущего прогона: [evidence](evidence/2026-10-12-gallery-contexts.json).

## Сброс группы профилей — 2026-10-12

- [x] Screen8slots или cooling4slots заменяются одной library revision; соседние
  группы и файлы сохраняются, invalid binding/stale edit не изменяют библиотеку.
- [x] CLI отправляет один revision-checked edit, не выполняет device Apply.
- [x] Desktop confirmation/cancel и lost-reply readback без повторной мутации;
  после ошибки keyboard focus переходит на обновление списка.
- [x] Полный Screen reset: durable recovery settings/slots/intent/collections
  реализован; результаты actor/IPC/GUI и lifecycle приведены ниже.
- [ ] Native keyboard/speech и device acceptance новой операции (позже).

[Checkpoint](evidence/2026-10-12-profile-group-reset.json).

## Screen reset storage transaction — 2026-10-12

- [x] Journal publication → settings write → profile write: recovery после
  каждого шага через fresh store и любой reader заканчивает обе записи.
- [x] Corrupt/future journal и внешний конфликт не уничтожают файлы/evidence.
- [x] Receipt/lost-reply retry сохраняет позднейшие queued deltas; rejected
  revision не теряет соседние pending settings.
- [x] Общий gate и fsync не блокируют async executor; current-thread regression
  охватывает reset и обычный debounced flush.
- [x] Подключить LCD actor (следующий checkpoint).
- [x] GUI внешнего Screen reset подключён; native qualification остаётся ниже.
- [ ] Проверить stop/commit/Apply, transport error/retry, concurrent Apply,
  sleep/detach и native readback на общей реализации; физически — позже.

[Checkpoint](evidence/2026-10-12-screen-reset-transaction.json).

## LCD actor Screen reset — 2026-10-12

- [x] Durable commit предшествует reset transport; Application50/0 и базовая
  тема применяются одной actor operation. Старые preparation tickets не проходят.
- [x] Поздний кадр/error прежнего renderer не заменяет результат reset.
- [x] Duplicate/lost-reply не повторяют Apply и сохраняют позднейшую яркость.
- [x] Config/frame failure сохраняют receipt/intent, сообщают lcd.last_error.
- [x] Неопределимое recovery блокирует мутации, сохраняя readback.
- [x] Active FFmpeg composition reset: codec reap до ответа, остановка старого
  video output, освобождение media pins до manager shutdown (MockBackend).
- [ ] Native qualification полного reset. GUI, коллекции и внешний readback/reapply реализованы.
- [x] Reset → sleep/detach/restart с MockBackend; физическая приёмка остаётся в M1.

[Checkpoint](evidence/2026-10-12-screen-reset-actor.json).

## Коллекции Screen: оставшаяся сквозная приёмка

- [x] IPC read/edit различает accepted revision и durable saved/error;
  потерянный ответ восстанавливается readback, без повторной мутации.
- [x] Legacy общий каталог переносится без потери user assets, включая
  конкурентный import; превышение лимита не приводит к скрытой обрезке.
- [x] GUI различает удаление из коллекции и физическое удаление файла;
  сохранённый файл можно вернуть через полный каталог (browser acceptance).
- [ ] Тот же re-add после полного Screen reset: native acceptance.
- [x] После failed save assets защищены до retry; после shutdown/restart
  protection опирается на durable references.
- [ ] Полный reset с collection revision, shared slots scope, receipt и
  explicit reapply проходит native success/error/cancel/keyboard сценарии.

[Membership owner checkpoint](evidence/2026-10-12-screen-collections-owner.json): 748 tests passed, 1 ignored; fmt, strict Clippy и docs прошли. Внешний IPC/GUI ещё открыт.

## Legacy migration: software gates

- [x] RPC переносит user images во все три контекста и сохраняет builtin contexts.
- [x] Import после подготовки отклоняет устаревший snapshot до записи;
  повторная подготовка включает новый asset.
- [x] Каталог сверх лимита отклоняется целиком, файлы не меняются.
- [x] Повтор migration после инициализации не сбрасывает коллекции.
- [x] GUI migration/readback/error/focus и добавление последующих импортов
  в выбранную коллекцию; это ещё не native acceptance.

[Legacy migration checkpoint](evidence/2026-10-12-screen-collections-migration.json): 752 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

## Панель коллекций: browser acceptance

- [x] Перенос legacy, protected defaults, remove → save → add back без file delete.
- [x] Delayed save → close/reopen → старый read → settlement → свежий read;
  controls заблокированы до settlement/readback.
- [x] Потеря ответа после commit и failed readback: повторная мутация недоступна;
  ручное чтение восстанавливает актуальный состав без повторного set.
- [x] После удаления строки keyboard focus возвращается к refresh control.
- [x] Выбор темы в существующих галереях использует сохранённый состав;
  import/re-add и разные устройства не смешивают коллекции.
- [ ] Native WebKitGTK/AT-SPI и screen reader для панели коллекций.

## Collection-aware gallery/import: browser acceptance

- [x] Image/background/watermark показывают свой сохранённый состав.
- [x] Ошибка чтения membership скрывает старые entries вместо полного каталога.
- [x] Video backgrounds относятся к выбранному устройству; A → B не смешивает списки.
- [x] Полный каталог позволяет rename/delete вне коллекции, но не Apply/add в тему.
- [x] Image import обновляет membership до draft; ошибка сохраняет файл, не меняет draft и не повторяет set автоматически.
- [x] Video import после A → B добавляется в коллекцию исходного A.
- [ ] Native WebKitGTK: коллекции → галереи → import → reset/replay и accessibility.


## Внешний Screen reset/readback — 2026-10-12

- [x] RPC возвращает durable receipt и LCD outcome; readback сериализован с reset.
- [x] Invalid ID и stale collection revision отклоняются; повтор ID не применяет reset заново.
- [x] Задержавшийся reset после нового Clear/Apply отклоняется до durable write.
- [x] Duplicate admission во время новой theme preparation не отменяет её ticket.
- [x] Explicit transport reapply с защитой более поздних изменений.
- [x] GUI scope/confirmation и ambiguous reply recovery (browser acceptance ниже).
- [x] Native keyboard/reset/reapply/AT-SPI полного Screen reset (см. последний checkpoint).
- [ ] Расширенные speech/scaling и ambiguous reply сценарии Screen reset.
- [x] Последовательный reset → sleep/detach/restart с MockBackend.
- [ ] Прямые in-flight detach/shutdown и аппаратный lifecycle.

[Checkpoint](evidence/2026-10-12-screen-reset-ipc.json).


## Reset reapply: software acceptance — 2026-10-12

- [x] Config/frame failure → explicit reapply → Theme; durable revisions неизменны.
- [x] Новый brightness patch или pending Apply отменяет подготовленный retry.
- [x] Повторный transport failure сохраняет возможность новой явной попытки.
- [x] Lost successful reply → ordered readback показывает успех, без повторной отправки.
- [x] RPC без текущего failed reset отклоняется; readback явно сообщает eligibility.
- [x] GUI выводит durable reset отдельно от LCD failure, позволяет readback/reapply.
- [ ] Native/accessibility и combined restart/detach/sleep; аппаратная проверка позже.

[Checkpoint](evidence/2026-10-12-screen-reset-reapply.json).


## Screen reset GUI: browser acceptance — 2026-10-12

- [x] Явная область сброса общих слотов, подтверждение и отмена без записи.
- [x] Durable receipt отдельно от LCD failure; кнопка explicit reapply.
- [x] Lost reply/readback failure блокирует новые мутации; автоматического retry нет.
- [x] Явный retry сохраняет operation ID и исходные expected revisions.
- [x] Revision rejection требует свежего подтверждения.
- [x] Pending reset переживает смену вкладки, блокирует Apply/смену устройства.
- [x] Позднее подтверждение инвалидирует ранее прочитанные профили/коллекции.
- [x] Коллекция не теряет свой редактор из-за собственного уведомления о save.
- [ ] Native WebKitGTK: reset/reapply/readback, disconnect/reconnect, close/reopen,
  keyboard/focus/speech/scaling; общий reset lifecycle и физический LCD позже.

[Screen reset GUI checkpoint](evidence/2026-10-12-screen-reset-gui.json).


## Screen reset: manager lifecycle — 2026-10-12

- [x] Завершённый reset → sleep/wake → новый attachment: receipt/config/theme восстановлены.
- [x] Detach → новый USB path с прежним serial: те же defaults/receipt, новое attachment ID.
- [x] Новый PersistenceStore и daemon: default theme, 50%/0°/Application подтверждены mock transport.
- [x] Повтор исходного reset ID после replay не меняет generation/revisions.
- [x] Восемь общих slots и collections согласованы; fan slot, соседние settings и пользовательский файл сохранены.
- [x] Failed reset config → sleep/wake: replay восстанавливает тему, старый retry недоступен.
- [x] Sleep во время committed reset (см. следующий checkpoint).
- [ ] Прямые in-flight detach/shutdown; физическая приёмка отдельно.

[Checkpoint](evidence/2026-10-12-screen-reset-lifecycle.json).


## Screen reset in-flight sleep — 2026-10-12

- [x] Durable reset удержан в config write; отмена RPC caller не отменяет actor work.
- [x] Sleep не подтверждается, пока write удержан; после release завершается тот же запрос.
- [x] После teardown старая LCD queue закрыта; wake восстанавливает default/receipt.
- [x] Повтор исходного operation ID после wake не выполняет reset повторно.
- [ ] Native reset/lifecycle и прямые in-flight detach/shutdown сценарии; hardware позже.

[Checkpoint](evidence/2026-10-12-screen-reset-inflight-sleep.json).


## Screen reset: native WebKitGTK — 2026-10-12

- [x] Клавиатура: раскрыть панель → отменить → подтвердить reset через реальный IPC.
- [x] Mock JPEG write failure после commit: receipt сохранён, ошибка и explicit reapply доступны.
- [x] Удаление fault → keyboard reapply → Theme, brightness50%/rotation0°, revision без повторного reset.
- [x] AT-SPI: readback button enabled/focused, reapply button enabled; focus возвращается после операции.
- [x] Каталог изображений и имя локального черновика сохраняются; восемь Screen entries после reset.
- [ ] Native ambiguous reset reply, expanded scaling/Wayland и speech; video/font contents и полный draft отдельными assertions.

[Native checkpoint](evidence/2026-10-12-screen-reset-native.json): 440 trusted keys,
13 AT-SPI snapshots, 37 live events; MockBackend, X11/100%. Это не hardware acceptance.


## GA2LCD Editor

[Отдельный контракт и полный editor checklist](ga2lcd-editor.md).
Geometry preparation, import editor GUI и publication подключены; browser
проверяет crop/order, stale preview, cancel и interrupted publication.
Gallery re-edit подключён с revision/readback; цветовые filters, video editor, capture и native приёмка
ещё не закрыты. Этот срез не является acceptance всего LCD-009.
