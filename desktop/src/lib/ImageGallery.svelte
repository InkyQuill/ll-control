<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { ValidationError, type UiDiagnostic } from './validation-error';
  import { t, diagnosticText } from './i18n.svelte';
  import { tick } from 'svelte';
  import ImageEditor from './ImageEditor.svelte';
  import { readCollectionMembership, imageInCollection } from './collection-membership';
  import { rpc, message, type ScreenCollections } from './api';
  let {
    device,
    collectionEpoch = 0,
    connected,
    disabled = false,
    addDisabled = false,
    keep = [],
    onedit,
    onadd,
  }: {
    device: string;
    collectionEpoch?: number;
    connected: boolean;
    disabled?: boolean;
    addDisabled?: boolean;
    keep?: string[];
    onedit: (key: string, context: Context, png: number[], name: string) => Promise<string>;
    onadd: (key: string, context: Context) => void;
  } = $props();
  type Context = 'image' | 'background' | 'watermark';
  let context = $state<Context>('image');
  let catalogMode = $state(false);
  interface AssetInfo {
    key: string;
    name: string | null;
    revision: number;
    is_default?: boolean;
    contexts?: Context[];
  }
  let assets = $state<AssetInfo[]>([]);
  let membership = $state<ScreenCollections | null>(null);
  const keys = $derived(
    assets
      .filter(
        (asset) =>
          (!asset.contexts || asset.contexts.includes(context)) &&
          (catalogMode || imageInCollection(membership, asset.key, context)),
      )
      .map((asset) => asset.key),
  );
  const builtin = (key: string) => assets.find((asset) => asset.key === key)?.is_default === true;
  const title = (key: string) =>
    assets.find((asset) => asset.key === key)?.name ?? t('Изображение {0}', key.slice(0, 8));
  type Diagnostic = { detail: string };
  let edit = $state<AssetInfo | null>(null),
    saving = $state(false),
    renameError = $state<Diagnostic | null>(null),
    renameReadback = $state(false);
  let confirmDelete = $state<AssetInfo | null>(null),
    deleteNotice = $state<string | Diagnostic>(''),
    deleteReadback = $state(false);
  let summaryElement: HTMLButtonElement | undefined = $state();
  let generation = 0,
    mutationEpoch = 0;
  $effect(() => {
    void device;
    void collectionEpoch;
    void connected;
    void open;
    void context;
    generation++;
    edit = null;
    renameError = null;
    renameReadback = false;
    confirmDelete = null;
    deleteNotice = '';
  });
  async function publishEdit(key: string, png: number[], name: string) {
    const target = device,
      purpose = context,
      focused = document.activeElement;
    try {
      return await onedit(key, purpose, png, name);
    } finally {
      await tick();
      if (
        device === target &&
        context === purpose &&
        open &&
        focused instanceof HTMLElement &&
        !focused.isConnected &&
        document.activeElement === document.body
      )
        summaryElement?.focus();
    }
  }
  async function rename() {
    if (
      !edit ||
      edit.is_default ||
      saving ||
      disabled ||
      !connected ||
      loading ||
      renameReadback ||
      deleteReadback ||
      !edit.name?.trim()
    )
      return;
    const request = { ...edit },
      started = generation;
    saving = true;
    renameError = null;
    renameReadback = true;
    try {
      await rpc('rename_theme_asset', {
        key: request.key,
        name: request.name,
        expected_revision: request.revision,
      });
      if (started === generation) edit = null;
    } catch (e) {
      if (started === generation) renameError = { detail: message(e) };
    } finally {
      saving = false;
      if (started === generation) revision++;
    }
  }
  async function deleteImage() {
    if (
      !confirmDelete ||
      confirmDelete.is_default ||
      saving ||
      disabled ||
      !connected ||
      deleteReadback ||
      keep.includes(confirmDelete.key)
    )
      return;
    const target = { ...confirmDelete },
      started = generation;
    mutationEpoch++;
    saving = true;
    deleteReadback = true;
    deleteNotice = '';
    confirmDelete = null;
    edit = null;
    try {
      await rpc('delete_theme_asset', {
        key: target.key,
        expected_revision: target.revision,
        keep: [...keep],
      });
      if (started === generation) deleteNotice = 'Изображение удалено. Исходный файл сохранён.';
    } catch (e) {
      if (started === generation) deleteNotice = { detail: message(e) };
    } finally {
      mutationEpoch++;
      saving = false;
      // A reopened gallery may have read the old list while deletion was pending.
      // Always invalidate it after settlement; only notices/focus are generation-bound.
      revision++;
      await tick();
      if (started === generation) summaryElement?.focus();
    }
  }
  let open = $state(false),
    page = $state(0),
    revision = $state(0);
  let loading = $state(false),
    error = $state<UiDiagnostic>('');
  let previews = $state<Record<string, string>>({}),
    failures = $state<Record<string, string>>({});
  const pages = $derived(Math.max(1, Math.ceil(keys.length / 8)));
  const currentPage = $derived(Math.min(page, pages - 1));
  const visible = $derived(keys.slice(currentPage * 8, (currentPage + 1) * 8));
  $effect(() => {
    void revision;
    void collectionEpoch;
    const target = device;
    if (!open || !connected) {
      assets = [];
      return;
    }
    let active = true;
    const epoch = mutationEpoch;
    loading = true;
    error = '';
    assets = [];
    Promise.all([rpc<AssetInfo[]>('list_named_theme_assets'), readCollectionMembership(target)])
      .then(([result, collections]) => {
        if (active && epoch === mutationEpoch) {
          assets = result;
          membership = collections;
          if (!saving) deleteReadback = false;
        }
      })
      .catch((e) => {
        if (active && epoch === mutationEpoch) {
          error = e instanceof ValidationError ? e : message(e);
          assets = [];
        }
      })
      .finally(() => {
        if (active && epoch === mutationEpoch) loading = false;
      });
    return () => {
      active = false;
    };
  });
  $effect(() => {
    const selected = visible;
    void revision;
    if (!open || !connected) return;
    let active = true;
    const urls: string[] = [];
    previews = {};
    failures = {};
    async function load() {
      for (const key of selected) {
        if (!active) break;
        try {
          const bytes = await rpc<number[]>('thumbnail_theme_asset', { key });
          if (!active) break;
          const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/png' }));
          urls.push(url);
          previews[key] = url;
        } catch (e) {
          if (active) failures[key] = message(e);
        }
      }
    }
    void load();
    return () => {
      active = false;
      urls.forEach((url) => URL.revokeObjectURL(url));
    };
  });
</script>

<StudioPopover
  label={t('Библиотека изображений')}
  bind:open
  bind:trigger={summaryElement}
  icon="image"
  text
  wide
>
  <StudioPopover label={t('Справка')} icon="help"
    ><p>
      {t(
        'Добавьте сохранённое изображение в черновик. Вывод на LCD изменится после применения темы.',
      )}
    </p></StudioPopover
  >
  {#if !connected}<p role="status">{t('Библиотека недоступна без связи с daemon.')}</p>
  {:else}
    <label
      ><input type="checkbox" bind:checked={catalogMode} />{t(
        'Показать все файлы изображений для управления',
      )}</label
    >
    <label
      >{t('Назначение изображения')}<select bind:value={context} disabled={saving}>
        <option value="image">{t('Слой изображения')}</option><option value="background"
          >{t('Фон')}</option
        ><option value="watermark">{t('Водяной знак')}</option>
      </select></label
    >
    <StudioPopover label={t('Справка')} icon="help"
      ><p>
        {context === 'background'
          ? '' +
            t(
              'Выбранное изображение заменит фон, сохранив все слои. Видео для фона доступно в библиотеке роликов.',
            ) +
            ''
          : context === 'watermark'
            ? '' +
              t(
                'Водяной знак добавится поверх остальных слоёв. Размер, положение и прозрачность можно изменить в редакторе.',
              ) +
              ''
            : '' + t('Изображение добавится отдельным слоем.') + ''}
      </p></StudioPopover
    >
    <button disabled={loading || saving} onclick={() => revision++}
      >{t('Обновить изображения')}</button
    >
    {#if error}<p role="alert">
        {t('Не удалось загрузить изображения:')}
        {diagnosticText(error)}
      </p>{/if}
    {#if loading}<p role="status">{t('Загрузка изображений…')}</p>
    {:else if !keys.length && !error}<p>
        {t('Сохранённых изображений пока нет. Импортируйте файл кнопкой «+ Изображение».')}
      </p>{/if}
    <div class="images">
      {#each visible as key (key)}
        <article>
          <p>{title(key)}</p>
          {#if !imageInCollection(membership, key, context)}<StudioPopover
              label={t('Справка')}
              icon="help"
              ><p>
                {t(
                  'Файл вне выбранной коллекции. Добавьте его через «Коллекции LCD» для выбора в тему.',
                )}
              </p></StudioPopover
            >{/if}
          {#if builtin(key)}<p>{t('Встроенное изображение')}</p>{/if}
          {#if previews[key]}<img src={previews[key]} alt={title(key)} width="144" height="144" />
          {:else if failures[key]}<p role="status">{t('Миниатюра недоступна:')} {failures[key]}</p>
          {:else}<p role="status">{t('Подготовка миниатюры…')}</p>{/if}
          <button
            disabled={!imageInCollection(membership, key, context) ||
              disabled ||
              (addDisabled && context !== 'background') ||
              saving ||
              deleteReadback ||
              !!failures[key] ||
              !previews[key]}
            onclick={() => onadd(key, context)}
            >{context === 'background'
              ? t('Сделать фоном {0}', title(key))
              : context === 'watermark'
                ? t('Добавить водяной знак {0}', title(key))
                : t('Добавить в тему {0}', builtin(key) ? title(key) : key.slice(0, 8))}</button
          >
          {#key `${device}:${context}:${key}`}
            <ImageEditor
              existing={{ key, name: title(key) }}
              disabled={disabled ||
                saving ||
                deleteReadback ||
                builtin(key) ||
                !imageInCollection(membership, key, context)}
              onpublish={(png, name) => publishEdit(key, png, name)}
            />
          {/key}
          <button
            disabled={disabled || saving || loading || deleteReadback || builtin(key)}
            onclick={() => {
              confirmDelete = null;
              edit = { ...assets.find((asset) => asset.key === key)! };
              renameError = null;
              renameReadback = false;
            }}>{t('Переименовать')} {title(key)}</button
          >
          <button
            disabled={disabled ||
              saving ||
              loading ||
              deleteReadback ||
              builtin(key) ||
              keep.includes(key)}
            onclick={() => {
              confirmDelete = { ...assets.find((asset) => asset.key === key)! };
              edit = null;
              deleteNotice = '';
            }}>{t('Удалить')} {title(key)}…</button
          >
          {#if keep.includes(key)}<p>{t('Изображение используется в черновике темы.')}</p>{/if}
        </article>
      {/each}
    </div>
    {#if confirmDelete}
      <p>
        {t('Удалить «')}{confirmDelete.name ??
          t('Изображение {0}', confirmDelete.key.slice(0, 8))}{t(
          '» из библиотеки? Исходный файл останется на диске.',
        )}
      </p>
      <button
        disabled={disabled || saving || deleteReadback || keep.includes(confirmDelete.key)}
        onclick={deleteImage}>{t('Подтвердить удаление изображения')}</button
      >
      <button disabled={saving} onclick={() => (confirmDelete = null)}
        >{t('Отмена удаления изображения')}</button
      >
    {/if}
    {#if deleteNotice}<p role="status">
        {typeof deleteNotice === 'string'
          ? t(deleteNotice)
          : t(
              'Удаление не подтверждено: {0}. Проверьте обновлённый список перед повтором.',
              deleteNotice.detail,
            )}
      </p>{/if}
    {#if edit}<form
        onsubmit={(event) => {
          event.preventDefault();
          void rename();
        }}
      >
        <label
          >{t('Имя изображения')}<input
            bind:value={edit.name}
            required
            maxlength="256"
            disabled={saving}
          /></label
        >
        <button
          disabled={saving ||
            disabled ||
            !connected ||
            loading ||
            renameReadback ||
            !edit.name?.trim()}>{t('Сохранить имя изображения')}</button
        >
        <button
          type="button"
          disabled={saving}
          onclick={() => {
            edit = null;
            renameError = null;
            renameReadback = false;
          }}>{t('Отмена переименования')}</button
        >
      </form>{/if}
    {#if renameError}<p role="alert">
        {t(
          'Переименование не подтверждено: {0}. Обновлённые данные будут загружены; перед повтором выберите переименование заново.',
          renameError.detail,
        )}
      </p>{/if}
    {#if keys.length}<nav aria-label={t('Страницы изображений')}>
        <button disabled={currentPage === 0} onclick={() => (page = currentPage - 1)}
          >{t('Предыдущие изображения')}</button
        >
        <span role="status">{t('Страница')} {currentPage + 1} {t('из')} {pages}</span>
        <button disabled={currentPage + 1 >= pages} onclick={() => (page = currentPage + 1)}
          >{t('Следующие изображения')}</button
        >
      </nav>{/if}
  {/if}
</StudioPopover>

<style>
  .images {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 160px), 1fr));
    gap: 12px;
  }
  article {
    min-width: 0;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 10px;
  }
  img {
    display: block;
    max-width: 100%;
    height: 144px;
    object-fit: contain;
  }
  p,
  button {
    overflow-wrap: anywhere;
  }
  nav {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
  }
</style>
