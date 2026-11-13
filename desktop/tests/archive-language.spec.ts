import { test, expect } from '@playwright/test';
import { policySummary, groupPhases, groupNames } from '../src/lib/group-archive';
import { archivePhase, profileGroupNames } from '../src/lib/profile-archive';
import {
  ArchiveUiError,
  archiveError,
  archiveMessage,
  archiveDiagnostic,
} from '../src/lib/archive-messages';
import { translate } from '../src/lib/localization';
import type { CoolingPolicy } from '../src/lib/api';

test('archive phase/name resources have English translations and preserve their Russian resources', () => {
  for (const source of [
    ...Object.values(groupPhases),
    ...Object.values(archivePhase),
    ...Object.values(groupNames),
    ...Object.values(profileGroupNames),
  ]) {
    expect(translate(source, 'en-US')).not.toMatch(/[А-Яа-яЁё]/);
    expect(translate(source, 'ru-RU')).toBe(source);
  }
});

test('cooling summaries translate their structure while retaining exact source IDs and manual zero', () => {
  const source = 'Настройки {1} <CPU>';
  const points = [
    { celsius: 30, percent: 40 },
    { celsius: 70, percent: 100 },
  ];
  for (const mode of ['curve', 'fan_curve', 'curve_rpm'] as const) {
    const policy: CoolingPolicy =
      mode === 'curve_rpm'
        ? {
            mode,
            sensor_id: source,
            points: points.map((p) => ({ celsius: p.celsius, rpm: p.percent * 30 })),
            hysteresis_c: 2,
            fallback_percent: 100,
          }
        : { mode, sensor_id: source, points, hysteresis_c: 2, fallback_percent: 100 };
    const unit = mode === 'curve_rpm' ? 'RPM' : 'PWM';
    expect(policySummary(policy, 'en-US')).toBe(`${unit} curve · ${source} · points: 2`);
    expect(policySummary(policy, 'ru-RU')).toBe(`Кривая ${unit} · ${source} · точек: 2`);
  }
  expect(policySummary({ mode: 'fixed', percent: 0 }, 'en-US')).toBe('0%');
  expect(policySummary({ mode: 'motherboard', percent: 70 }, 'en-US')).toBe('Motherboard');
  expect(policySummary({ mode: 'motherboard', percent: 70 }, 'ru-RU')).toBe('Материнская плата');
  expect(policySummary({ mode: 'fixed_rpm', rpm: 2400 }, 'en-US')).toBe('2400 RPM');
  expect(policySummary(null, 'en-US')).toBe('not configured');
  expect(policySummary(null, 'ru-RU')).toBe('не задана');
});

test('typed archive notices translate by code while legacy/future/arbitrary diagnostics remain exact data', () => {
  const local = archiveError(new ArchiveUiError('pending_group_operation'));
  expect(archiveMessage(local, 'en-US')).toBe(
    'Read the saved operation result and close its details first.',
  );
  expect(archiveMessage(local, 'ru-RU')).toBe(
    'Сначала прочитайте результат сохранённой операции и закройте её сведения.',
  );
  const raw = 'Настройки {1} <CPU>.lcp';
  const value = archiveError({ message: raw, message_code: 'destination_exists' });
  expect(archiveMessage(value, 'en-US')).toBe('The file already exists. Choose a new name.');
  expect(archiveMessage(value, 'ru-RU')).toBe('Файл уже существует: выберите новое имя');
  expect(archiveDiagnostic(value, 'en-US')).toBe(raw);
  expect(archiveDiagnostic(value, 'ru-RU')).toBe(raw);
  for (const message_code of [undefined, 'future_code', '__proto__']) {
    const unknown = archiveError({ message: 'Настройки', message_code });
    expect(archiveMessage(unknown, 'en-US')).toBe('Настройки');
    expect(archiveDiagnostic(unknown, 'en-US')).toBe('');
  }
  for (const code of [
    'cancelled',
    'profile_export_unconfirmed',
    'group_export_unconfirmed',
    'cancel_unconfirmed',
    'invalid_archive_file',
    'invalid_profile_archive',
    'invalid_manifest_length',
    'destination_exists',
  ]) {
    expect(archiveMessage({ message: null, message_code: code }, 'en-US')).not.toMatch(
      /[А-Яа-яЁё]/,
    );
    expect(archiveMessage({ message: null, message_code: code }, 'ru-RU')).toMatch(/[А-Яа-яЁё]/);
  }
});
