import { fireworksColors } from './api';
import { ValidationError } from './validation-error';
import type { Theme, LcdPageKind, WeatherEffect } from './api';
import { assertWeatherDraft } from './weather-settings';
export function isProcedural(layer: import('./api').Layer | undefined): boolean {
  return (
    layer?.kind === 'weather' ||
    layer?.kind === 'meteor' ||
    layer?.kind === 'snow3d' ||
    layer?.kind === 'fireworks'
  );
}
export function lcdPageKind(theme: Theme): LcdPageKind {
  if (theme.layers.some((layer) => layer.kind === 'fireworks')) return 'fireworks';
  if (theme.layers.some((layer) => layer.kind === 'snow3d')) return 'snow3d';
  if (theme.layers.some((layer) => layer.kind === 'meteor')) return 'meteor';
  const weather = theme.layers.find((layer) => layer.kind === 'weather');
  return weather?.effect.kind ?? 'basic';
}
export function themePages(theme: Theme): Theme[] {
  return [theme, ...(theme.page_cache ?? []).map((page) => page.theme)].flatMap((page) => [
    page,
    ...(page.basic_editor?.banks ?? []).flatMap((bank) => bank.overlays.map((item) => item.theme)),
  ]);
}
function snapshot(theme: Theme): Theme {
  const copy = structuredClone(theme);
  delete copy.page_cache;
  const weather = copy.layers.find((layer) => layer.kind === 'weather');
  if (weather) delete weather.effect.presets;
  return copy;
}
export function switchLcdPage(theme: Theme, target: LcdPageKind): Theme {
  if (lcdPageKind(theme) === target) return theme;
  assertWeatherDraft(theme);
  const current = snapshot(theme),
    cache = [...(theme.page_cache ?? [])];
  // Upgrade scalar weather presets created before full page-state storage.
  for (const page of themePages(theme)) {
    for (const preset of page.layers.find((layer) => layer.kind === 'weather')?.effect.presets ??
      []) {
      if (preset.kind === lcdPageKind(theme) || cache.some((saved) => saved.page === preset.kind))
        continue;
      const migrated = snapshot(page);
      const weather = migrated.layers.find((layer) => layer.kind === 'weather');
      if (weather) {
        weather.effect = structuredClone(preset);
        cache.push({ page: preset.kind, theme: migrated });
      }
    }
  }
  const saved = cache.find((page) => page.page === target);
  let next: Theme;
  if (saved) next = snapshot(saved.theme);
  else {
    next = snapshot(theme);
    next.layers = next.layers.filter((layer) => !isProcedural(layer));
    if (target !== 'basic') {
      delete next.basic_editor;
      if (next.layers.length >= 32)
        throw new ValidationError('Для нового эффекта нужен свободный слой.');
      if (target === 'meteor')
        next.layers.unshift({
          kind: 'meteor',
          effect: {
            speed: 2,
            colors: [
              [0, 255, 0],
              [0, 0, 255],
            ],
            dynamic: false,
            sensor: null,
            seed: 42,
          },
        });
      else if (target === 'fireworks')
        next.layers.unshift({
          kind: 'fireworks',
          effect: {
            intensity: 1,
            colors: fireworksColors(),
            dynamic: false,
            sensor: null,
            seed: 42,
          },
        });
      else if (target === 'snow3d')
        next.layers.unshift({ kind: 'snow3d', effect: { intensity: 1, seed: 42 } });
      else {
        const legacy = theme.layers
          .find((layer) => layer.kind === 'weather')
          ?.effect.presets?.find((preset) => preset.kind === target);
        const effect: WeatherEffect = legacy
          ? structuredClone(legacy)
          : { kind: target, intensity: 1, dynamic: false, sensor: null, seed: 42 };
        next.layers.unshift({ kind: 'weather', effect });
      }
    }
    next.version = target === 'basic' ? (next.background_video ? 2 : 1) : 3;
  }
  next.page_cache = [
    ...cache.filter((page) => page.page !== target),
    { page: lcdPageKind(current), theme: current },
  ];
  return next;
}
