<script lang="ts">
 import {presentation} from './presentation.svelte';
 import {temperatureFromCelsius,temperatureToCelsius} from './temperature';
 let {value=$bindable(),min,max,step='any',difference=false,disabled=false,invalid=false,curve=false}:{value:number|undefined;min:number;max:number;step?:number|'any';difference?:boolean;disabled?:boolean;invalid?:boolean;curve?:boolean}=$props();
 const unit=$derived(presentation.settings?.temperature_unit??'celsius');
 const displayMin=$derived(temperatureFromCelsius(min,unit,difference));
 const displayMax=$derived(temperatureFromCelsius(max,unit,difference));
</script>
<!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK native Value interface requires an explicit spinbutton role.) -->
<input type="number" role="spinbutton" data-curve-temperature={curve?'':undefined} {disabled} min={displayMin} max={displayMax} step={step==='any'?'any':temperatureFromCelsius(step,unit,true)} aria-valuemin={displayMin} aria-valuemax={displayMax} aria-valuenow={value!==undefined&&Number.isFinite(value)?temperatureFromCelsius(value,unit,difference):undefined} aria-invalid={invalid||value===undefined||!Number.isFinite(value)||value<min||value>max} bind:value={()=>value!==undefined&&Number.isFinite(value)?Number(temperatureFromCelsius(value,unit,difference).toFixed(8)):undefined,displayed=>value=temperatureToCelsius(displayed,unit,difference)}/>
