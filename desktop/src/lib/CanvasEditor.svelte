<script lang="ts">
  import { t } from './i18n.svelte';
  import type { Layer } from './api';
  import {
    editableLayer,
    layerTransform,
    nativePoint,
    resizeLayer,
    translateLayer,
    type LayerBounds,
    type QuarterTurn,
  } from './canvas-geometry';

  let {
    image,
    images = [],
    background = '',
    keys,
    layers,
    bounds,
    selected,
    rotation = 0,
    ready,
    labels,
    onselect,
    onchange,
    onorder,
  }: {
    image: string;
    images?: string[];
    background?: string;
    keys: number[];
    layers: Layer[];
    bounds: (LayerBounds | null)[];
    selected: number;
    rotation: QuarterTurn;
    ready: boolean;
    labels: Record<Layer['kind'], string>;
    onselect: (index: number) => void;
    onchange: (index: number, layer: Layer) => void;
    onorder: (index: number, step: number) => void;
  } = $props();
  let surface: HTMLDivElement;
  type Gesture = {
    id: number;
    index: number;
    layer: Layer;
    bounds: LayerBounds;
    start: [number, number];
    mode: 'move' | 'resize';
    dx: number;
    dy: number;
  };
  let gesture = $state<Gesture | null>(null);
  $effect(() => {
    if (
      gesture &&
      (!ready ||
        selected !== gesture.index ||
        JSON.stringify(layers[gesture.index]) !== JSON.stringify(gesture.layer))
    )
      cancel();
  });
  function point(event: PointerEvent): [number, number] {
    const rect = surface.getBoundingClientRect();
    return nativePoint(event.clientX - rect.left, event.clientY - rect.top, rect.width, rotation);
  }
  function start(event: PointerEvent, index: number, mode: 'move' | 'resize') {
    if (
      !ready ||
      layers[index].editor?.locked ||
      layers[index].editor?.hidden ||
      event.button !== 0 ||
      !bounds[index]
    )
      return;
    event.preventDefault();
    event.stopPropagation();
    onselect(index);
    (event.currentTarget as HTMLElement).focus();
    const original = JSON.parse(JSON.stringify(layers[index])) as Layer;
    gesture = {
      id: event.pointerId,
      index,
      layer: original,
      bounds: { ...bounds[index]! },
      start: point(event),
      mode,
      dx: 0,
      dy: 0,
    };
    surface.setPointerCapture(event.pointerId);
  }
  function motion(event: PointerEvent) {
    if (!gesture || event.pointerId !== gesture.id) return;
    const position = point(event);
    gesture.dx = position[0] - gesture.start[0];
    gesture.dy = position[1] - gesture.start[1];
  }
  function finish(event: PointerEvent) {
    if (!gesture || event.pointerId !== gesture.id) return;
    motion(event);
    const g = gesture;
    gesture = null;
    if (surface.hasPointerCapture(event.pointerId)) surface.releasePointerCapture(event.pointerId);
    if (Math.abs(g.dx) + Math.abs(g.dy) < 0.5) return;
    if (g.mode === 'move') translateLayer(g.layer, g.dx, g.dy);
    else resizeLayer(g.layer, g.bounds, g.dx, g.dy);
    onchange(g.index, g.layer);
  }
  function cancel() {
    const id = gesture?.id;
    gesture = null;
    if (id !== undefined && surface?.hasPointerCapture(id)) surface.releasePointerCapture(id);
  }
  function keyboard(event: KeyboardEvent, index: number) {
    if (event.key === 'Escape') {
      event.preventDefault();
      cancel();
      return;
    }
    if (!ready || layers[index].editor?.locked || gesture || !bounds[index]) return;
    const directions: Record<string, [number, number]> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const direction = directions[event.key];
    if (!direction) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.altKey) {
      onorder(index, direction[0] || -direction[1]);
      return;
    }
    const layer = JSON.parse(JSON.stringify(layers[index])) as Layer;
    const step =
      event.ctrlKey || event.metaKey
        ? 10
        : layer.kind === 'image' && layer.watermark_position
          ? 5
          : 1;
    // Direction keys follow the visible, rotated canvas.
    const origin = nativePoint(0, 0, 480, rotation),
      endpoint = nativePoint(direction[0] * step, direction[1] * step, 480, rotation);
    let dx = endpoint[0] - origin[0],
      dy = endpoint[1] - origin[1];
    if (event.shiftKey && 'style' in layer) {
      dx *= bounds[index]!.width / layer.style.size;
      dy *= bounds[index]!.height / layer.style.size;
    }
    if (event.shiftKey) resizeLayer(layer, bounds[index]!, dx, dy);
    else translateLayer(layer, dx, dy);
    onchange(index, layer);
  }
  function transform(index: number) {
    const g = gesture?.index === index ? gesture : null;
    return g
      ? layerTransform(JSON.parse(JSON.stringify(g.layer)), g.bounds, g.mode, g.dx, g.dy)
      : { dx: 0, dy: 0, sx: 1, sy: 1, ox: 0, oy: 0 };
  }
  function rectangle(index: number, b: LayerBounds): string {
    const t = transform(index);
    const x = t.ox + (b.x - t.ox) * t.sx + t.dx,
      y = t.oy + (b.y - t.oy) * t.sy + t.dy;
    return `left:${x / 4.8}%;top:${y / 4.8}%;width:${Math.max(4, b.width * t.sx) / 4.8}%;height:${Math.max(4, b.height * t.sy) / 4.8}%`;
  }
  function pixelTransform(index: number): string {
    const t = transform(index);
    return `translate(${t.dx / 4.8}%,${t.dy / 4.8}%) scale(${t.sx},${t.sy})`;
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === 'Escape') cancel();
  }}
  onblur={cancel}
/>
<div
  class="canvas"
  bind:this={surface}
  role="group"
  aria-label={t('Холст LCD')}
  onpointermove={motion}
  onpointerup={finish}
  onpointercancel={cancel}
  onlostpointercapture={cancel}
>
  <div class="rotated" style:transform={`rotate(${rotation}deg)`}>
    {#if gesture && background && images.length === layers.length}
      <img src={background} alt={t('Черновик LCD')} draggable="false" />
      {#each layers as layer, index (keys[index])}
        {#if images[index] && !layer.editor?.hidden}
          <img
            class="layer-pixels"
            src={images[index]}
            alt=""
            draggable="false"
            style:transform={pixelTransform(index)}
            style:transform-origin={`${transform(index).ox / 4.8}% ${transform(index).oy / 4.8}%`}
          />
        {/if}
      {/each}
    {:else if image}<img src={image} alt={t('Черновик LCD')} draggable="false" />{/if}
    {#each layers as layer, index (keys[index])}
      {@const box = bounds[index]}
      {#if box && !layer.editor?.hidden && editableLayer(layer)}
        <button
          class="region"
          class:selected={selected === index}
          style={rectangle(index, box)}
          aria-label={t('Выбрать слой {0}: {1}', index + 1, labels[layer.kind])}
          aria-pressed={selected === index}
          aria-disabled={!ready}
          onfocus={() => onselect(index)}
          onclick={() => onselect(index)}
          onpointerdown={(event) => start(event, index, 'move')}
          onkeydown={(event) => keyboard(event, index)}
        >
          {#if selected === index}<span class="number">{index + 1}</span>{/if}
        </button>
      {/if}
    {/each}
    {#if selected >= 0 && bounds[selected] && layers[selected] && !layers[selected].editor?.locked && !layers[selected].editor?.hidden && editableLayer(layers[selected])}
      {@const box = bounds[selected]!}
      <div class="selection" style={rectangle(selected, box)}></div>
      <button
        class="resize"
        style:left={`${Math.max(6, Math.min(474, box.x + box.width)) / 4.8}%`}
        style:top={`${Math.max(6, Math.min(474, box.y + box.height)) / 4.8}%`}
        aria-label={t('Изменить размер выбранного слоя')}
        aria-disabled={!ready}
        onpointerdown={(event) => start(event, selected, 'resize')}
        onkeydown={(event) => keyboard(event, selected)}
      ></button>
    {/if}
  </div>
  {#if !ready}<span class="pending">{t('Обновление холста…')}</span>{/if}
</div>

<style>
  .canvas {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    touch-action: none;
    overflow: hidden;
    isolation: isolate;
  }
  .rotated {
    position: absolute;
    inset: 0;
    transform-origin: center;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    user-select: none;
  }
  .region {
    position: absolute;
    display: block;
    min-width: 0;
    min-height: 0;
    padding: 0;
    margin: 0;
    border: 1px solid transparent;
    border-radius: 0;
    background: transparent;
    cursor: move;
    touch-action: none;
  }
  .region:hover,
  .region:focus-visible {
    border-color: #fff;
    background: #ffffff12;
    outline: 2px solid #101820;
    outline-offset: 1px;
  }
  .region.selected {
    border-color: #7ce9ff;
  }
  .number {
    position: absolute;
    left: 0;
    top: 0;
    color: #071018;
    background: #7ce9ff;
    font-size: 11px;
    line-height: 14px;
    padding: 0 3px;
    pointer-events: none;
  }
  .selection {
    position: absolute;
    pointer-events: none;
    border: 1px solid #7ce9ff;
    box-sizing: border-box;
  }
  .resize {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 12px;
    height: 12px;
    min-width: 0;
    min-height: 0;
    padding: 0;
    border: 2px solid #071018;
    border-radius: 2px;
    background: #7ce9ff;
    pointer-events: auto;
    cursor: nwse-resize;
    touch-action: none;
  }
  .resize:focus-visible {
    outline: 2px solid white;
    outline-offset: 2px;
  }
  .pending {
    position: absolute;
    bottom: 6px;
    left: 6px;
    font-size: 12px;
    background: #101820dd;
    color: white;
    padding: 3px 6px;
    pointer-events: none;
  }
</style>
