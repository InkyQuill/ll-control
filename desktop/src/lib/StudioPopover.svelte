<script lang="ts">
  import {onMount,type Snippet} from 'svelte';
  import StudioIcon from './StudioIcon.svelte';
  import {t} from './i18n.svelte';
  let {label,icon='settings',children,open=$bindable(false),text=false,wide=false,trigger=$bindable()}: {label:string;icon?:string;children:Snippet;open?:boolean;text?:boolean;wide?:boolean;trigger?:HTMLButtonElement}=$props();
  const id=$props.id();
  let panel:HTMLDivElement|undefined=$state();
  function position(){
    if(!panel||!trigger)return;
    const r=trigger.getBoundingClientRect();
    const width=Math.min(wide?720:380,window.innerWidth-32);
    panel.style.width=`${width}px`;
    panel.style.left=`${Math.max(16,Math.min(r.left,window.innerWidth-width-16))}px`;
    const height=Math.min(panel.scrollHeight,window.innerHeight-48);
    const top=window.innerHeight-r.bottom-24>=height?r.bottom+8:r.top-height-8;
    panel.style.top=`${Math.max(16,Math.min(top,window.innerHeight-height-16))}px`;
  }
  onMount(()=>{
    const ancestors:HTMLElement[]=[];
    for(let node=trigger?.parentElement;node;node=node.parentElement)ancestors.push(node);
    const observer=new MutationObserver(()=>{if(ancestors.some(node=>node.hidden))panel?.hidePopover();});
    for(const node of ancestors)observer.observe(node,{attributes:true,attributeFilter:['hidden']});
    const resize=()=>{if(open)position();};
    const sizes=new ResizeObserver(resize);if(panel)sizes.observe(panel);
    window.addEventListener('resize',resize);
    return ()=>{observer.disconnect();sizes.disconnect();window.removeEventListener('resize',resize);};
  });
  $effect(()=>{if(panel){if(open)panel.showPopover();else panel.hidePopover();}});
</script>
<button bind:this={trigger} type="button" class="popover-trigger" class:help={icon==='help'} aria-label={label} title={label} popovertarget={id} onclick={position}><StudioIcon name={icon}/>{#if text}<span>{label}</span>{/if}</button>
<div bind:this={panel} {id} popover="auto" class="studio-popover" role="dialog" aria-label={label} onbeforetoggle={event=>{if(event.newState==='open')queueMicrotask(position);}} ontoggle={event=>{open=event.newState==='open';if(open)position();}}>
  <header><h2>{label}</h2><button type="button" class="icon-action" aria-label={t('Закрыть')} title={t('Закрыть')} popovertarget={id} popovertargetaction="hide"><StudioIcon name="close"/></button></header>
  {@render children()}
</div>
<style>
  .popover-trigger{display:inline-flex;align-items:center;justify-content:center;gap:8px;width:auto;min-width:36px;padding:8px;border-color:transparent;background:transparent;vertical-align:middle}.help{width:28px;min-width:28px;min-height:28px;padding:4px;color:var(--text-muted)}
  .studio-popover{position:fixed;inset:auto;margin:0;max-width:calc(100vw - 32px);max-height:calc(100vh - 48px);overflow:auto;padding:16px;background:var(--surface-raised);color:var(--text);border:0;border-radius:12px;box-shadow:0 12px 32px #0008}
  header{display:flex;align-items:center;justify-content:space-between;gap:16px;margin-bottom:12px}h2{font-size:15px;margin:0}header button{flex:none}
  .studio-popover :global(label){margin-block:8px}.studio-popover :global(p){font-size:13px;line-height:1.5;color:var(--text-muted)}
</style>
