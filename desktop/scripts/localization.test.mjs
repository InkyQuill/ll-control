import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  symlinkSync,
  realpathSync,
  rmSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { translate } from '../src/lib/localization.ts';

test('unknown keys that name inherited object properties remain literal UI fallback text', () => {
  for (const language of ['en-US', 'ru-RU'])
    for (const key of ['constructor', 'toString', '__proto__', 'hasOwnProperty', 'valueOf']) {
      assert.equal(translate(key, language), key);
      assert.equal(translate(` ${key} `, language), ` ${key} `);
    }
});
test('catalog resources interpolate filenames as literal values', () => {
  const name = 'constructor {0} <clip>';
  assert.equal(translate('First frame: {0}', 'en-US', name), 'First frame: ' + name);
  assert.equal(translate('First frame: {0}', 'ru-RU', name), 'Первый кадр: ' + name);
});
test('strict catalog audit requires own dictionary entries and reports original TypeScript lines', () => {
  const root = mkdtempSync(join(tmpdir(), 'llcontrol-locale-contract-'));
  try {
    const scripts = join(root, 'desktop/scripts'),
      lib = join(root, 'desktop/src/lib'),
      routes = join(root, 'desktop/src/routes');
    mkdirSync(scripts, { recursive: true });
    mkdirSync(join(lib, 'locales'), { recursive: true });
    mkdirSync(routes, { recursive: true });
    symlinkSync(
      realpathSync(new URL('../node_modules', import.meta.url)),
      join(root, 'desktop/node_modules'),
      'dir',
    );
    writeFileSync(
      join(scripts, 'check-locales.mjs'),
      readFileSync(new URL('./check-locales.mjs', import.meta.url)),
    );
    writeFileSync(join(routes, '+page.svelte'), '<p>Fixture</p>\n');
    const keys = ['constructor', 'toString', '__proto__'];
    writeFileSync(
      join(lib, 'owned.ts'),
      '// original module line mapping\n' +
        keys
          .map(
            (k, index) =>
              `throw new ${index % 2 ? 'Error' : 'ValidationError'}(${JSON.stringify(k)});`,
          )
          .join('\n'),
    );
    const dictionary = join(lib, 'locales/ru-RU.json'),
      report = join(root, 'report.json');
    writeFileSync(dictionary, '{}');
    const run = () =>
      spawnSync(
        process.execPath,
        [join(scripts, 'check-locales.mjs'), '--strict', '--report', report],
        { encoding: 'utf8' },
      );
    const missing = run();
    assert.equal(missing.status, 1, missing.stderr);
    const result = JSON.parse(readFileSync(report, 'utf8'));
    assert.equal(result.missingStaticKeys, 3);
    for (const [index, key] of keys.entries())
      assert.deepEqual(result.missing.find((r) => r.key === key).locations, [
        { file: 'desktop/src/lib/owned.ts', line: index + 2 },
      ]);
    writeFileSync(
      dictionary,
      JSON.stringify(Object.fromEntries(keys.map((k) => [k, `Own ${k} entry`]))),
    );
    const translated = run();
    assert.equal(translated.status, 0, translated.stderr);
    assert.equal(JSON.parse(readFileSync(report, 'utf8')).missingStaticKeys, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

async function ownedValidation(run) {
  let error;
  try {
    await run();
  } catch (cause) {
    error = cause;
  }
  assert.equal(error?.name, 'ValidationError');
  return error;
}
function localized(error, language) {
  return translate(error.resource, language, ...error.values);
}
function localizedPair(russian, english) {
  return (error) => {
    assert.equal(localized(error, 'ru-RU'), russian);
    assert.equal(localized(error, 'en-US'), english);
  };
}
async function viteModule(path, run) {
  const { createServer } = await import('vite');
  const server = await createServer({
    configFile: false,
    root: new URL('..', import.meta.url).pathname,
    server: { middlewareMode: true },
    appType: 'custom',
    logLevel: 'silent',
  });
  try {
    return await run(await server.ssrLoadModule(path));
  } finally {
    await server.close();
  }
}

test('oversized image validation is localized in both interface languages', async () => {
  await viteModule('/src/lib/api.ts', async ({ importImageAsset }) => {
    const file = new File([new Uint8Array(16 * 1024 * 1024 + 1)], 'oversized.png');
    const error = await ownedValidation(() => importImageAsset(file));
    localizedPair('Изображение превышает 16 MiB', 'Image exceeds 16 MiB')(error);
  });
});

test('every effect draft validation path has localized EN/RU resources', async () => {
  await viteModule('/src/lib/weather-settings.ts', async ({ assertWeatherDraft }) => {
    const theme = (layer) => ({
      version: 3,
      name: 'Validation',
      background: [0, 0, 0],
      layers: [layer],
    });
    const binding = (low = 30, high = 60, unit = 'percent') => ({
      sensor_id: 'cpu',
      unit,
      low,
      high,
    });
    const cases = [
      [
        theme({ kind: 'fireworks', effect: { dynamic: true, sensor: null, intensity: 1 } }),
        'Выберите источник эффекта перед сохранением или применением.',
        'Select an effect source before saving or applying.',
      ],
      [
        theme({ kind: 'fireworks', effect: { dynamic: false, sensor: null, intensity: 0 } }),
        'Проверьте интенсивность и пороги Fireworks.',
        'Check the Fireworks intensity and thresholds.',
      ],
      [
        theme({
          kind: 'fireworks',
          effect: { dynamic: true, sensor: { binding: binding(-1) }, intensity: 1 },
        }),
        'Проверьте интенсивность и пороги Fireworks.',
        'Check the Fireworks intensity and thresholds.',
      ],
      [
        theme({ kind: 'snow3d', effect: { intensity: 4 } }),
        'Проверьте интенсивность Snow3D: 1–3.',
        'Check the Snow3D intensity: 1–3.',
      ],
      [
        theme({ kind: 'meteor', effect: { dynamic: true, sensor: null, speed: 2 } }),
        'Выберите источник эффекта перед сохранением или применением.',
        'Select an effect source before saving or applying.',
      ],
      [
        theme({ kind: 'meteor', effect: { dynamic: false, sensor: null, speed: 0 } }),
        'Проверьте скорость и пороги MeteorShower.',
        'Check the MeteorShower speed and thresholds.',
      ],
      [
        theme({
          kind: 'meteor',
          effect: { dynamic: true, sensor: { binding: binding(0, 3601, 'rpm') }, speed: 2 },
        }),
        'Проверьте скорость и пороги MeteorShower.',
        'Check the MeteorShower speed and thresholds.',
      ],
      [
        theme({ kind: 'weather', effect: { dynamic: true, sensor: null, intensity: 1 } }),
        'Выберите источник эффекта перед сохранением или применением.',
        'Select an effect source before saving or applying.',
      ],
      [
        theme({
          kind: 'weather',
          effect: { dynamic: true, sensor: binding(80, 20), intensity: 1 },
        }),
        'Проверьте пороги эффекта: 0–100 для °C/% и 0–3600 для RPM; нижний не должен превышать верхний.',
        'Check the effect thresholds: 0–100 for °C/% and 0–3600 for RPM; the lower threshold must not exceed the upper threshold.',
      ],
      [
        theme({
          kind: 'weather',
          effect: { dynamic: true, sensor: binding(0, 3601, 'rpm'), intensity: 1 },
        }),
        'Проверьте пороги эффекта: 0–100 для °C/% и 0–3600 для RPM; нижний не должен превышать верхний.',
        'Check the effect thresholds: 0–100 for °C/% and 0–3600 for RPM; the lower threshold must not exceed the upper threshold.',
      ],
    ];
    for (const [draft, russian, english] of cases) {
      const error = await ownedValidation(() => assertWeatherDraft(draft));
      localizedPair(russian, english)(error);
    }
  });
});

test('new-effect layer limit is localized in both interface languages', async () => {
  await viteModule('/src/lib/lcd-pages.ts', async ({ switchLcdPage }) => {
    const layers = Array.from({ length: 32 }, (_, index) => ({
      kind: 'text',
      text: String(index),
      style: { x: 0, y: 0, size: 12, color: [0, 0, 0], align: 'center' },
    }));
    const error = await ownedValidation(() =>
      switchLcdPage({ version: 1, name: 'Full', background: [0, 0, 0], layers }, 'meteor'),
    );
    localizedPair(
      'Для нового эффекта нужен свободный слой.',
      'A free layer is required for the new effect.',
    )(error);
  });
});

test('English is the literal source language and Russian is a separate catalog', () => {
  assert.equal(translate('First frame: {0}', 'en-US', 'clip'), 'First frame: clip');
  assert.equal(
    translate('Unknown English resource {0}', 'ru-RU', 'clip'),
    'Unknown English resource clip',
  );
  const catalog = JSON.parse(
    readFileSync(new URL('../src/lib/locales/ru-RU.json', import.meta.url)),
  );
  for (const key of Object.keys(catalog)) assert.doesNotMatch(key, /[А-Яа-яЁё]/);
});

test('recording validation and Windows record labels have Russian resources', () => {
  for (const source of [
    'Invalid recording ID',
    'Recording editor is closed',
    'Invalid source preview size',
    'Invalid recording review ID',
    'Recording review is closed',
    'Recording is not ready to save',
    'Preview dimensions exceed preparation budget',
    'Canvas unavailable',
    'Unexpected preview dimensions',
  ]) {
    assert.equal(translate(source, 'en-US'), source);
    assert.match(translate(source, 'ru-RU'), /[А-Яа-яЁё]/);
  }
  assert.equal(translate('Record {0} · {1}', 'ru-RU', 2, 'abc123'), 'Запись 2 · abc123');
});

test('preset sensor requirements avoid count-dependent grammar and retain literal names', () => {
  for (const count of [1, 2, 5]) {
    assert.equal(
      translate('Sensors required for {0}: {1}', 'en-US', 'Focus {1}', count),
      `Sensors required for Focus {1}: ${count}`,
    );
    assert.equal(
      translate('Sensors required for {0}: {1}', 'ru-RU', 'Focus {1}', count),
      `Необходимое число датчиков для Focus {1}: ${count}`,
    );
  }
});
