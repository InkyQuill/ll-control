import type { BackgroundMode } from '../src/lib/api.ts';
import { createServer } from 'vite';
const server = await createServer({
  configFile: false,
  root: new URL('..', import.meta.url).pathname,
  server: { middlewareMode: true },
  appType: 'custom',
  logLevel: 'silent',
});
try {
  const { presets, presetTheme } = await server.ssrLoadModule('/src/lib/theme-presets.ts');
  const { createBasicEditor } = await server.ssrLoadModule('/src/lib/basic-editor.ts');
  const themes = presets.map((preset) =>
    presetTheme(
      preset.id,
      Array.from({ length: preset.sensors }, (_, i) => ({
        id: `cpu:${i}`,
        label: `Датчик ${i}`,
        unit: 'celsius' as const,
        value: 42,
        status: 'valid',
        age_ms: 10,
      })),
    ),
  );
  themes.push(
    createBasicEditor(presetTheme('clock', []), [
      { id: 'cpu:0', label: 'CPU', unit: 'celsius', value: 42, status: 'valid', age_ms: 10 },
    ]),
  );
  for (const unit of ['volts', 'hertz', 'bytes_per_second', 'bytes', 'percent'] as const) {
    const sensor = { id: `gpu:${unit}`, label: unit, unit, value: 1, status: 'valid', age_ms: 10 };
    themes.push(presetTheme('focus', [sensor]), presetTheme('cycle', [sensor, sensor, sensor]));
  }
  for (const mode of ['solid', 'horizontal', 'vertical', 'radial'] as BackgroundMode[]) {
    const theme = presetTheme('clock', []);
    const settings = () => ({
      colors: [
        [52, 230, 209],
        [26, 115, 105],
      ] as [number[], number[]],
      flip: true,
    });
    theme.background_fill = {
      mode,
      horizontal: settings(),
      vertical: settings(),
      radial: settings(),
    };
    for (const layer of theme.layers) {
      if (layer.kind !== 'image') {
        layer.style.line_box = true;
        layer.style.line_height = 58;
      }
    }
    themes.push(theme);
  }
  for (const font of [
    'noto_sans_tc_regular',
    'noto_sans_tc_bold',
    'abril_fatface',
    'caprasimo',
    'cubic11',
  ] as const) {
    const theme = presetTheme('clock', []);
    for (const layer of theme.layers) {
      if (layer.kind !== 'image') layer.style.font = font;
    }
    themes.push(theme);
  }
  const managed = presetTheme('clock', []);
  for (const layer of managed.layers) {
    if (layer.kind !== 'image') layer.style.font_asset = '0123456789abcdef0123456789abcdef';
  }
  themes.push(managed);
  const video = presetTheme('clock', []);
  video.version = 2;
  video.background_video = '0123456789abcdef0123456789abcdef';
  themes.push(video);
  for (const fit of ['crop', 'contain', 'contain_native', 'stretch'] as const) {
    const image = presetTheme('clock', []);
    image.background_image = { asset: 'builtin-midnight-v1', fit };
    image.layers.push({
      kind: 'image',
      asset: 'builtin-orbit-mark-v1',
      x: 0,
      y: 0,
      width: 144,
      height: 144,
      fit: 'contain_native',
      opacity: 255,
      watermark_position: { horizontal: 50, vertical: 50 },
    });
    themes.push(image);
  }
  for (const kind of ['rain', 'snow'] as const) {
    for (const dynamic of [false, true]) {
      for (const videoBackground of [false, true]) {
        const weather = presetTheme('clock', []);
        weather.version = 3;
        if (videoBackground) weather.background_video = '0123456789abcdef0123456789abcdef';
        weather.layers.unshift({
          kind: 'weather',
          effect: {
            kind,
            intensity: 3,
            dynamic,
            seed: 42,
            sensor: { sensor_id: 'cpu:weather', unit: 'celsius', low: 30, high: 75 },
            retained_sensors: [
              { sensor_id: 'gpu:weather', unit: 'percent', low: 15, high: 90 },
              { sensor_id: 'pump:weather', unit: 'rpm', low: 800, high: 1200 },
            ],
          },
        });
        if (weather.layers[0].kind === 'weather') {
          weather.layers[0].effect.presets = [
            {
              kind: kind === 'rain' ? 'snow' : 'rain',
              intensity: 2,
              dynamic: false,
              sensor: null,
              seed: 123,
            },
          ];
        }
        themes.push(weather);
      }
    }
  }
  process.stdout.write(JSON.stringify(themes));
} finally {
  await server.close();
}
