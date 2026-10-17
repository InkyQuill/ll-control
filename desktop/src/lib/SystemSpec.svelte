<script lang="ts">
  import {t} from './i18n.svelte';
  import {presentation} from './presentation.svelte';
  import {untrack} from 'svelte';
  import {rpc, message, type SystemSpec, type SpecKind, type SpecValue} from './api';
  let {connected}: {connected:boolean} = $props();
  const kinds: {kind:SpecKind;label:string}[] = [
    {kind:'cpu',label:'CPU'}, {kind:'gpu',label:'GPU'}, {kind:'memory',label:'Память'},
    {kind:'monitors',label:'Мониторы'}, {kind:'motherboard',label:'Материнская плата'}, {kind:'storage',label:'Накопители'}
  ];
  const labels: Record<string,string> = {model:'Модель',vendor:'Производитель',manufacturer:'Производитель',socket:'Сокет / пакет',logical_threads:'Логические потоки',physical_cores:'Физические ядра',vendor_id:'PCI Vendor ID',device_id:'PCI Device ID',driver:'Драйвер',revision:'Ревизия',os_visible_capacity:'Память, доступная ОС',reported_modules:'Доступные записи модулей',part_number:'Номер модели модуля',locator:'Расположение',bank:'Банк',memory_type:'Тип памяти',capacity:'Ёмкость',rated_transfer_rate:'Паспортная скорость',configured_transfer_rate:'Настроенная скорость',connector:'Разъём',product_id:'Код продукта',physical_size:'Физический размер',detailed_timing:'Первый подробный видеорежим EDID',version:'Версия',bios_vendor:'Производитель BIOS',bios_version:'Версия BIOS',bios_date:'Дата BIOS',device:'Устройство',firmware:'Прошивка',media:'Тип носителя'};
  let snapshot = $state<SystemSpec|null>(null), kind = $state<SpecKind>('cpu'), selectedId = $state('');
  type SpecError = {kind:'unsupported'} | {kind:'backend';diagnostic:string};
  let busy = $state(false), error = $state<SpecError|null>(null), fresh = $state(false);
  let generation = 0;
  const group = $derived(snapshot?.groups.find(g=>g.kind===kind));
  const item = $derived(group?.items.find(i=>i.id===selectedId) ?? group?.items[0]);
  function format(value:SpecValue):string {
    if(value.kind==='text') return value.value;
    if(value.kind==='count') return value.value.toLocaleString(presentation.language);
    if(value.kind==='hertz') return `${value.value.toLocaleString(presentation.language)} Hz`;
    const units=['B','KiB','MiB','GiB','TiB','PiB'];
    const index=value.value>0?Math.min(Math.floor(Math.log(value.value)/Math.log(1024)),units.length-1):0;
    return `${(value.value/1024**index).toLocaleString(presentation.language,{maximumFractionDigits:2})} ${units[index]}`;
  }
  async function load(refresh:boolean) {
    const request=++generation; busy=true; error=null;
    try {
      const next=await rpc<SystemSpec>('get_system_spec',{refresh});
      if(request!==generation) return;
      if(!next || next.version!==1 || next.groups.length!==6 || kinds.some(k=>next.groups.filter(g=>g.kind===k.kind).length!==1)) {error={kind:'unsupported'};fresh=false;return;}
      snapshot=next; fresh=true;
    } catch(e) {if(request===generation){error={kind:'backend',diagnostic:message(e)};fresh=false;}}
    finally {if(request===generation) busy=false;}
  }
  $effect(()=>{
    const online=connected;
    generation++; fresh=false; busy=false;
    if(!online) return;
    const timer=setTimeout(()=>{void untrack(()=>load(false));},0);
    return ()=>{clearTimeout(timer);generation++;};
  });
</script>
<section aria-label="{t("Сведения о системе")}" aria-busy={busy}>
  <div class="heading"><h2>{t("Сведения о системе")}</h2><button disabled={!connected || busy} onclick={()=>load(true)}>{t("Обновить сведения")}</button></div>
  <p>{t("Статические характеристики оборудования. Подключение помпы не требуется.")}</p>
  {#if !connected}<p role="status">{t("Нет подключения к daemon. Сохранённые сведения могут быть устаревшими.")}</p>{/if}
  {#if error}<p role="alert">{t("Не удалось получить сведения:")} {error.kind==='unsupported'?t('Неподдерживаемый ответ сведений о системе'):error.diagnostic}{t(". Повторите обновление.")}</p>{/if}
  {#if snapshot && !fresh && connected}<p role="status">{t("Показаны предыдущие сведения; актуальность не подтверждена.")}</p>{/if}
  {#if busy}<p role="status">{t("Читаем сведения об оборудовании…")}</p>{/if}
  <div class="groups" role="group" aria-label="{t("Категории оборудования")}">
    {#each kinds as category}<button aria-pressed={kind===category.kind} onclick={()=>{kind=category.kind;selectedId='';}}>{t(category.label)}</button>{/each}
  </div>
  {#if snapshot}
    <p class="updated">{t("Получено:")} {new Date(snapshot.collected_at_unix_ms).toLocaleString(presentation.language)}</p>
    {#if !group?.items.length}<p role="status">{t("Сведения в этой категории недоступны.")}</p>
    {:else}
      <label>{t("Компонент")}<select value={item?.id} onchange={event=>selectedId=event.currentTarget.value}>{#each group.items as entry (entry.id)}<option value={entry.id}>{entry.title}</option>{/each}</select></label>
      {#if item}<h3>{item.title}</h3><dl>{#each item.fields as field (field.key)}<div><dt>{Object.hasOwn(labels,field.key)?t(labels[field.key]):field.key}</dt><dd><span>{field.value===null?"" + t("Недоступно") + "":format(field.value)}</span><small>{t("Источник:")} {field.source}</small></dd></div>{/each}</dl>{/if}
      {#if kind==='memory'}<p>{t("Доступная ОС память отличается от суммы установленных модулей. Характеристики модулей показываются только при доступности SMBIOS.")}</p>{/if}
    {/if}
  {:else if !busy && connected && !error}<p>{t("Сведения ещё не загружены.")}</p>{/if}
</section>
<style>
  section{max-width:900px}.heading{display:flex;gap:16px;align-items:center;justify-content:space-between}.heading h2{margin:0}.groups{display:flex;flex-wrap:wrap;gap:8px;margin:24px 0}button[aria-pressed=true]{background:var(--accent);border-color:var(--accent)}.updated,small{color:var(--text-muted);font-size:12px}label{max-width:580px}dl{margin:20px 0}dl>div{display:grid;grid-template-columns:minmax(140px,1fr) minmax(0,2fr);gap:18px;padding:12px 0;border-bottom:1px solid var(--border)}dd{margin:0;overflow-wrap:anywhere}small{display:block;margin-top:5px}p{line-height:1.6}
</style>
