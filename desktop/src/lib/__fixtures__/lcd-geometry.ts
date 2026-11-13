import { createRawSnippet, mount, unmount } from 'svelte';
import LcdPreview from '../LcdPreview.svelte';
import DeviceGlyph from '../DeviceGlyph.svelte';
import type { DisplayGeometry } from '../device-capabilities';

const svg =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 480"><rect width="480" height="480" fill="#12121a"/><path d="M0 0h100v100H0z" fill="#ff694d"/><path d="M380 0h100v100H380z" fill="#3bcbb0"/><path d="M0 380h100v100H0z" fill="#b09bff"/><path d="M380 380h100v100H380z" fill="#f1d284"/></svg>';

/** Component fixture only; no unsupported device family is registered. */
export function mountGeometry(geometry: DisplayGeometry) {
  const target = document.createElement('section');
  target.id = 'lcd-geometry-fixture';
  target.style.cssText =
    'position:fixed;top:20px;left:20px;width:320px;z-index:10000;background:var(--canvas)';
  document.body.append(target);
  const children = createRawSnippet(() => ({
    render: () =>
      `<img alt="Four corner markers" src="data:image/svg+xml,${encodeURIComponent(svg)}"/>`,
  }));
  const preview = mount(LcdPreview, { target, props: { geometry, children } });
  const glyph = mount(DeviceGlyph, { target, props: { display: geometry } });
  return async () => {
    await unmount(preview);
    await unmount(glyph);
    target.remove();
  };
}
