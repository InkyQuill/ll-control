<script lang="ts">
  import {tick, untrack} from 'svelte';
  import {t, diagnosticText} from './i18n.svelte';
  import {rpc, message, coolingModeDefault, type CoolingPolicy, type Sensor} from './api';
  import {coolingModes, coolingPolicyKey, coolingModeLabel, defaultCoolingModeName, supportsCoolingModeDefault, type DeviceCoolingModes, type DeviceCoolingMode, type CoolingModeReference} from './cooling-modes';
  import {ValidationError, type UiDiagnostic} from './validation-error';
  let {device, target, connected, disabled = false, onload, draft, onsaved, draftContext = () => 0, sensors = [], defaultSensor, draftStamp, onrestore}: {
    device: string; target: 'fan' | 'pump'; connected: boolean; disabled?: boolean; onload: (policy: CoolingPolicy, reference: CoolingModeReference) => void; draft: () => CoolingPolicy; onsaved?: (reference: CoolingModeReference) => void; draftContext?: () => number; sensors?: Sensor[]; defaultSensor:()=>string; draftStamp:()=>string; onrestore:(policy:CoolingPolicy)=>void
  } = $props();
  let library = $state.raw<DeviceCoolingModes | null>(null), loaded = $state(false), loading = $state(false);
  let selected = $state(''), error = $state<UiDiagnostic | null>(null);
  let editName = $state(''), editId = $state(''), editRevision = $state(''), saving = $state(false), errorContext = $state<'read'|'save'|'create'|'restore'>('read');
  let loadedProfile = $state.raw<DeviceCoolingMode|null>(null), creationNotice=$state<'created'|'found'|null>(null);
  let createSensor=$state(''), pendingCreate=$state(false), creating=$state(false);
  let editDefaultLabel=$state(''), editUsesDefaultName=$state(false);
  let restoring=$state(false), restoreNotice=$state<'name'|'profile'|'changed'|null>(null);
  const temperatures=$derived(sensors.filter(sensor=>sensor.unit==='celsius'));
  let readButton: HTMLButtonElement | undefined;
  let modeSelect = $state<HTMLSelectElement | undefined>(undefined);
  type SaveAttempt = {dev:string;target:'fan'|'pump';revision:string;profile:DeviceCoolingMode;context:number};
  let pendingSave = $state.raw<SaveAttempt | null>(null), savedSnapshot = $state.raw<DeviceCoolingMode | null>(null);
  let generation = 0;
  const modes = $derived(library?.[target] ?? []);
  const chosen = $derived(modes.find(mode => mode.id === selected));
  const factorySupported=$derived(!!chosen&&supportsCoolingModeDefault(chosen.id,target));
  const factoryLabel=$derived(chosen&&factorySupported?coolingModeLabel({...chosen,name:defaultCoolingModeName(chosen.id)!,use_default_name:true}):'');
  const factoryNeedsSource=$derived(!!chosen&&!(target==='pump'&&chosen.id==='FixRPM'));
  const factorySourceKnown=$derived(sensors.some(sensor=>sensor.id===defaultSensor()&&sensor.unit==='celsius'));
  const conflict = $derived(!pendingSave && !!editId && !!library && library.revision !== editRevision);
  const draftChanged = $derived.by(() => {
    if (!savedSnapshot) return false;
    try { return (savedSnapshot.use_default_name ? !editUsesDefaultName||editName!==editDefaultLabel : editName!==coolingModeLabel(savedSnapshot)) || coolingPolicyKey(draft()) !== coolingPolicyKey(savedSnapshot.policy); }
    catch { return true; }
  });
  let scope = '';
  $effect(() => {
    const nextScope = JSON.stringify([device,target]), online = connected;
    generation++;
    untrack(() => {
      library = null; loaded = false; loading = false; saving = false; loadedProfile = null; error = null;
      if (scope !== nextScope) {
        selected = ''; editName = ''; editId = ''; editRevision = ''; pendingSave = null; savedSnapshot = null; errorContext = 'read';pendingCreate=false;createSensor='';editUsesDefaultName=false;editDefaultLabel='';
      } else if (pendingSave || pendingCreate) {
        errorContext = pendingCreate?'create':'save';
        error = pendingCreate?new ValidationError('Ответ на создание не подтверждён. После подключения прочитайте режимы устройства.'):new ValidationError('Ответ на сохранение не подтверждён. После подключения прочитайте режимы устройства.');
      }
      scope = nextScope;restoring=false;restoreNotice=null;creating=false;creationNotice=null;
      // Keep an unresolved save across connection changes. Only an explicit read
      // of this same device/channel can reconcile it; never replay the write.
      if (!online) loaded = false;
    });
    return () => { generation++; };
  });
  async function refresh() {
    if (!device || !connected || disabled || loading || saving || restoring) return;
    const started = ++generation, dev = device, channel = target;
    const current = () => started === generation && device === dev && target === channel && connected;
    const attempt=pendingSave, creation=pendingCreate;
    loading = true; loaded = false; loadedProfile = null; error = null; errorContext='read';
    try {
      const value = await rpc<unknown>('get_cooling_modes',{dev});
      if (!current()) return;
      const snapshot=coolingModes(value);
      if(attempt) reconcile(snapshot,attempt,true);
      else if(creation) reconcileCreation(snapshot,true,false);
      else {
        library=snapshot;loaded=true;
        if(!snapshot?.[channel].some(mode=>mode.id===selected)){selected='';editId='';editRevision='';}
      }
    } catch (cause) {
      if (current()) {library=null;loaded=false;error = cause instanceof ValidationError ? cause : message(cause);}
    } finally { if (current()) loading = false; }
  }
  function load() {
    if (!chosen || !library || disabled || loading || saving || restoring || pendingSave || pendingCreate || !connected || !device) return;
    onload(chosen.policy,{id:chosen.id,revision:library.revision,policy:structuredClone(chosen.policy)}); loadedProfile = chosen;creationNotice=null;restoreNotice=null;
    editName=coolingModeLabel(chosen);editUsesDefaultName=!!chosen.use_default_name;editDefaultLabel=editName;editId=chosen.id;editRevision=library.revision;savedSnapshot=null;error=null;
  }
  function choose(id:string) {restoreNotice=null;creationNotice=null;selected=id;editUsesDefaultName=false;editDefaultLabel='';editName=modes.find(mode=>mode.id===id)?.name??'';editId='';editRevision='';loadedProfile=null;savedSnapshot=null;error=null;}
  function reconcile(snapshot:DeviceCoolingModes|null,attempt:SaveAttempt,explicit:boolean) {
    library=snapshot;loaded=true;selected=attempt.profile.id;errorContext='save';
    const mode=snapshot?.[attempt.target].find(mode=>mode.id===attempt.profile.id);
    const durable=!!snapshot&&!snapshot.persistence.pending&&snapshot.persistence.last_error===null;
    if(mode&&durable&&mode.name===attempt.profile.name&&!!mode.use_default_name===!!attempt.profile.use_default_name&&coolingPolicyKey(mode.policy)===coolingPolicyKey(attempt.profile.policy)) {
      pendingSave=null;savedSnapshot=attempt.profile;editId=mode.id;editRevision=snapshot!.revision;loadedProfile=null;error=null;if(attempt.context===draftContext())onsaved?.({id:mode.id,revision:snapshot!.revision,policy:structuredClone(attempt.profile.policy)});return;
    }
    if(snapshot&&!durable) error=snapshot.persistence.last_error
      ?new ValidationError('Сохранение настроек не подтверждено: {0}',[snapshot.persistence.last_error])
      :new ValidationError('Настройки ещё сохраняются. Повторите чтение.');
    else error=new ValidationError('Текущие данные режима отличаются от отправленного снимка. Черновик сохранён. Перечитайте режимы перед новым Save.');
    if(explicit&&(durable||snapshot===null)) {
      pendingSave=null;
      if(!mode){selected='';editId='';editRevision='';}
    }
  }
  function reconcileCreation(snapshot:DeviceCoolingModes|null,explicit:boolean,reply:boolean) {
    library=snapshot;loaded=true;errorContext='create';
    if(snapshot&&!snapshot.persistence.pending&&snapshot.persistence.last_error===null) {
      pendingCreate=false;creationNotice=reply?'created':'found';error=null;return;
    }
    if(snapshot) error=snapshot.persistence.last_error
      ?new ValidationError('Сохранение настроек не подтверждено: {0}',[snapshot.persistence.last_error])
      :new ValidationError('Настройки ещё сохраняются. Повторите чтение.');
    else {
      error=new ValidationError('Набор режимов пока отсутствует. Перечитайте данные перед повторным созданием.');
      if(explicit){pendingCreate=false;error=new ValidationError('Набор отсутствует. Перед повторным созданием проверьте выбранный источник.');}
    }
  }
  async function initialize() {
    if(!loaded||library!==null||disabled||!connected||!device||loading||saving||restoring||pendingSave||pendingCreate)return;
    if(!temperatures.some(sensor=>sensor.id===createSensor))return;
    const started=++generation,dev=device,channel=target,sensor=createSensor,focused=document.activeElement;
    const current=()=>started===generation&&device===dev&&target===channel&&connected;
    saving=true;creating=true;pendingCreate=true;error=null;errorContext='create';creationNotice=null;
    try {
      const snapshot=coolingModes(await rpc<unknown>('initialize_cooling_modes',{dev,sensor}));
      if(current())reconcileCreation(snapshot,false,true);
    }catch(cause){
      if(!current())return;
      try {const snapshot=coolingModes(await rpc<unknown>('get_cooling_modes',{dev}));if(current())reconcileCreation(snapshot,false,false);}
      catch {if(current())error=cause instanceof ValidationError?cause:message(cause);}
    }finally {
      if(current()) {
        saving=false;creating=false;await tick();
        if(current()&&(document.activeElement===focused||document.activeElement===document.body)) {
          const destination=pendingCreate?readButton:modeSelect??readButton;
          if(destination&&!destination.disabled)destination.focus();
        }
      }
    }
  }
  function restoreName() {
    if(!chosen||editId!==chosen.id||!factorySupported||disabled||loading||saving||restoring||pendingSave||pendingCreate||!connected)return;
    editName=factoryLabel;editDefaultLabel=editName;editUsesDefaultName=true;
    savedSnapshot=null;loadedProfile=null;error=null;restoreNotice='name';
  }
  async function restoreMode() {
    if(!chosen||!library||editId!==chosen.id||!factorySupported||disabled||loading||saving||restoring||pendingSave||pendingCreate||!connected||factoryNeedsSource&&!factorySourceKnown)return;
    const started=++generation,dev=device,channel=target,id=chosen.id,sensor=defaultSensor();
    const stamp=draftStamp(),context=draftContext(),name=editName,usesDefault=editUsesDefaultName;
    const current=()=>started===generation&&device===dev&&target===channel&&connected;
    restoring=true;error=null;errorContext='restore';restoreNotice=null;
    try {
      const value=await coolingModeDefault(channel,id,sensor);
      if(!current())return;
      let profile:DeviceCoolingMode|undefined;
      try {
        const bank=coolingModes({version:1,revision:'0'.repeat(64),fan:channel==='fan'?[value]:[],pump:channel==='pump'?[value]:[],persistence:{pending:false,saved:false,last_error:null}});
        profile=bank?.[channel][0];
      }catch {throw new ValidationError('Приложение вернуло неверный стандартный режим. Черновик сохранён.');}
      if(!profile||profile.id!==id||!profile.use_default_name)throw new ValidationError('Приложение вернуло неверный стандартный режим. Черновик сохранён.');
      if(draftStamp()!==stamp||draftContext()!==context||editName!==name||editUsesDefaultName!==usesDefault){restoreNotice='changed';return;}
      onrestore(profile.policy);editName=coolingModeLabel(profile);editDefaultLabel=editName;editUsesDefaultName=true;
      savedSnapshot=null;loadedProfile=null;restoreNotice='profile';
    }catch(cause){if(current())error=cause instanceof ValidationError?cause:message(cause);}
    finally {if(current())restoring=false;}
  }
  async function save() {
    if(!chosen||!library||editId!==chosen.id||pendingSave||pendingCreate||disabled||loading||saving||restoring||!connected)return;
    error=null;errorContext='save';
    let profile:DeviceCoolingMode;
    try {
      if(/^\p{White_Space}*$/u.test(editName)||new TextEncoder().encode(editName).length>128||/\p{Cc}/u.test(editName)) throw new ValidationError('Проверьте имя режима: оно должно быть коротким и содержать допустимые символы.');
      const factory=editUsesDefaultName&&editName===editDefaultLabel;
      profile={id:chosen.id,name:factory?defaultCoolingModeName(chosen.id)!:editName,policy:structuredClone(draft()),...(factory?{use_default_name:true}:{})};
    }catch(cause){error=cause instanceof ValidationError?cause:message(cause);return;}
    const attempt:SaveAttempt={dev:device,target,revision:library.revision,profile,context:draftContext()};
    const started=++generation;
    const current=()=>started===generation&&device===attempt.dev&&target===attempt.target&&connected;
    saving=true;restoreNotice=null;pendingSave=attempt;savedSnapshot=null;loadedProfile=null;
    try {
      const snapshot=coolingModes(await rpc<unknown>('save_cooling_mode',{dev:attempt.dev,target:attempt.target,expected_revision:attempt.revision,profile}));
      if(current())reconcile(snapshot,attempt,false);
    }catch(cause){
      if(!current())return;
      try {
        const snapshot=coolingModes(await rpc<unknown>('get_cooling_modes',{dev:attempt.dev}));
        if(current())reconcile(snapshot,attempt,false);
      }catch{if(current())error=cause instanceof ValidationError?cause:message(cause);}
    }finally{
      if(current()){
        saving=false;
        if(pendingSave){await tick();if(current())readButton?.focus();}
      }
    }
  }
</script>

<details class="device-modes">
  <summary>{t('Сохранённые режимы устройства')}</summary>
  <p>{t('Загрузите режим в черновик, чтобы проверить или изменить его перед Apply.')}</p>
  <div class="actions">
    <button bind:this={readButton} disabled={disabled || saving || restoring || !connected || !device} aria-disabled={loading} aria-busy={loading} onclick={refresh}>{loading ? t('Загрузка…') : t('Прочитать режимы устройства')}</button>
    {#if loaded && modes.length}
      <label>{t('Сохранённый режим')}<select bind:this={modeSelect} value={selected} onchange={e=>choose(e.currentTarget.value)} disabled={disabled || loading || saving || restoring || !!pendingSave || pendingCreate || !connected}>
        <option value="">{t('Выберите режим')}</option>
        {#each modes as mode (mode.id)}<option value={mode.id}>{coolingModeLabel(mode)}{mode.id !== mode.name ? ` (${mode.id})` : ''}</option>{/each}
      </select></label>
      <button disabled={disabled || loading || saving || restoring || !!pendingSave || pendingCreate || !connected || !chosen} onclick={load}>{t('Загрузить режим в черновик')}</button>
    {/if}
  </div>
  {#if loaded && library===null}
    <p>{t('Создайте стандартные 8 режимов вентиляторов и 3 режима помпы. Источник температуры выбирается явно; работающие настройки не изменятся.')}</p>
    <div class="actions">
      <label>{t('Источник стандартных режимов')}<select bind:value={createSensor} disabled={disabled||loading||saving||pendingCreate||!connected}>
        <option value="">{t('Выберите температурный источник')}</option>
        {#each temperatures as sensor}<option value={sensor.id}>{sensor.label} · {sensor.status}</option>{/each}
      </select></label>
      <button disabled={disabled||loading||!connected||pendingCreate&&!creating||!temperatures.some(sensor=>sensor.id===createSensor)} aria-disabled={creating} aria-busy={creating} onclick={initialize}>{creating?t('Создание…'):t('Создать стандартные режимы')}</button>
    </div>
  {/if}
  {#if creationNotice}<p role="status">{creationNotice==='created'?t('Стандартные режимы сохранены. Работающие настройки не изменены; выберите режим и загрузите его в черновик.'):t('Найдены сохранённые режимы. Проверьте список перед загрузкой; повторная запись не отправлялась.')}</p>{/if}
  {#if chosen && editId===chosen.id}
    <div class="actions edit">
      <label>{t('Имя режима')}<input bind:value={editName} oninput={()=>{editUsesDefaultName=false;}} disabled={disabled||loading||saving||!!pendingSave||!connected}/></label>
      <button disabled={disabled||loading||restoring||!connected||!!pendingSave&&!saving} aria-disabled={saving} aria-busy={saving} onclick={save}>{saving?t('Сохранение…'):conflict?t('Сохранить черновик поверх обновлённого режима'):t('Сохранить снимок режима')}</button>
    </div>
    {#if factorySupported}
      <div class="actions restore">
        <button disabled={disabled||loading||saving||restoring||!!pendingSave||pendingCreate||!connected||editUsesDefaultName&&editName===factoryLabel} onclick={restoreName}>{t('Вернуть стандартное имя')}</button>
        <button disabled={disabled||loading||saving||!!pendingSave||pendingCreate||!connected||factoryNeedsSource&&!factorySourceKnown} aria-disabled={restoring} aria-busy={restoring} onclick={restoreMode}>{restoring?t('Подготовка…'):t('Стандартные параметры в черновик')}</button>
      </div>
      {#if factoryNeedsSource&&!factorySourceKnown}<p>{t('Выберите температурный источник в редакторе, чтобы восстановить стандартную кривую.')}</p>{/if}
    {/if}
    <p>{t('Save меняет имя и сохранённую копию черновика, но не рабочую политику охлаждения.')}</p>
    {#if conflict}<p>{t('Сохранённые данные изменились. Загрузите актуальный режим или явно сохраните свой черновик поверх него.')}</p>{/if}
  {/if}
  {#if loaded && !modes.length}<p role="status">{t('Для этого устройства пока нет сохранённых режимов. Используйте пресеты или пользовательские профили.')}</p>{/if}
  {#if loaded && modes.length && !loadedProfile && !savedSnapshot}<p role="status">{t('Доступно сохранённых режимов: {0}',modes.length)}</p>{/if}
  {#if error}<p class="error" role="alert">{errorContext==='restore'?t('Не удалось подготовить стандартный режим: {0}. Черновик сохранён.',diagnosticText(error)):errorContext==='create'?t('Создание режимов не подтверждено: {0}',diagnosticText(error)):errorContext==='save'?t('Сохранение режима не подтверждено: {0}',diagnosticText(error)):t('Не удалось прочитать режимы охлаждения: {0}',diagnosticText(error))}</p>{/if}
  {#if restoreNotice}<p role="status">{restoreNotice==='name'?t('Стандартное имя восстановлено в черновике. Нажмите Save, чтобы сохранить его.'):restoreNotice==='profile'?t('Стандартные параметры загружены в черновик. Save сохранит их в режиме; Apply применит текущий черновик.'):t('Черновик изменился. Стандартные параметры не заменили ваши правки.')}</p>{/if}
  {#if savedSnapshot}<p role="status">{draftChanged?t('Сохранён снимок режима «{0}». В текущем черновике есть другие правки.',coolingModeLabel(savedSnapshot)):t('Снимок режима «{0}» сохранён. Рабочая политика не изменена.',coolingModeLabel(savedSnapshot))}</p>{/if}
  {#if loadedProfile}<p role="status">{t('Режим «{0}» загружен в черновик. Для отправки нажмите «Применить охлаждение».',coolingModeLabel(loadedProfile))}</p>{/if}
</details>

<style>
  .device-modes {border-block:1px solid var(--border);padding:14px 0}
  summary {cursor:pointer;font-weight:600}
  p {font-size:13px;line-height:1.6;color:var(--text-muted);max-width:65ch}
  .actions {display:flex;flex-wrap:wrap;gap:12px;align-items:end}
  .edit,.restore {margin-top:16px}
  label {display:grid;gap:6px;min-width:0;max-width:100%}
  select {max-width:100%}
  button[aria-disabled="true"] {color:var(--text-muted);cursor:progress}
  .error {color:var(--danger);overflow-wrap:anywhere}
</style>
