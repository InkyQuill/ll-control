import { test, expect } from '@playwright/test';

for (const kind of ['image', 'video'] as const)
  test(`${kind} import confirms membership once and preserves adjacent entries`, async ({
    page,
  }) => {
    await page.addInitScript(() => {
      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: async (command: string, args: Record<string, unknown> = {}) => {
            if (command !== 'daemon_call') return null;
            if (args.method === 'list_devices' || args.method === 'list_sensors') return [];
            if (args.method === 'get_display_settings')
              return {
                version: 1,
                revision: 0,
                temperature_unit: 'celsius',
                interface_language: 'en-US',
              };
            return null;
          },
        },
      });
    });
    await page.goto('/');
    const results = await page.evaluate(async (kind) => {
      const modulePath = '/src/lib/collection-membership.ts';
      const { addImportedToCollection } = await import(/* @vite-ignore */ modulePath);
      const internal = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__;
      const original = internal.invoke;
      const results = [];
      try {
        for (const mode of [
          'legacy',
          'existing',
          'confirmed',
          'newer',
          'stale',
          'missing',
          'null',
          'pending',
          'error',
        ]) {
          type Collections = {
            version: number;
            revision: number;
            images: string[];
            watermarks: string[];
            backgrounds: { kind: string; key: string }[];
          };
          let collections: Collections | null = {
            version: 1,
            revision: 4,
            images: ['image-neighbor'],
            watermarks: ['watermark-neighbor'],
            backgrounds: [
              { kind: 'image', key: 'background-neighbor' },
              { kind: 'video', key: 'video-neighbor' },
            ],
          };
          if (mode === 'legacy') collections = null;
          if (mode === 'existing' && collections) {
            if (kind === 'image') collections.images.push('new');
            else collections.backgrounds.push({ kind, key: 'new' });
          }
          let reads = 0,
            writes = 0,
            requested: unknown = null;
          internal.invoke = async (command, args) => {
            const params = args?.params as
              { dev?: string; expected_revision: number; collections: Collections } | undefined;
            if (command !== 'daemon_call' || params?.dev !== 'contract-device')
              return original(command, args);
            if (args.method === 'get_screen_collections') {
              reads++;
              if (writes && mode === 'error') throw { message: 'readback unavailable' };
              return {
                collections: structuredClone(collections),
                persistence: { pending: writes > 0 && mode === 'pending', last_error: null },
              };
            }
            if (args.method === 'set_screen_collections') {
              writes++;
              requested = structuredClone(params);
              if (mode !== 'stale') collections = structuredClone(params.collections);
              if (mode === 'null') collections = null;
              if (mode === 'missing' && collections) {
                collections.images = collections.images.filter((key) => key !== 'new');
                collections.backgrounds = collections.backgrounds.filter(
                  (item) => item.key !== 'new',
                );
              }
              if (mode === 'newer' && collections) {
                collections.revision++;
                collections.watermarks.push('concurrent-neighbor');
              }
              return params.collections;
            }
            throw new Error('Unexpected mutation: ' + args.method);
          };
          let error: string | null = null;
          try {
            await addImportedToCollection('contract-device', kind, 'new');
          } catch (e) {
            error = e instanceof Error ? e.message : String((e as { message: unknown }).message);
          }
          results.push({ mode, reads, writes, error, requested });
        }
      } finally {
        internal.invoke = original;
      }
      return results;
    }, kind);
    for (const result of results) {
      const early = ['legacy', 'existing'].includes(result.mode),
        success = early || ['confirmed', 'newer'].includes(result.mode);
      expect(result.reads, result.mode).toBe(early ? 1 : 2);
      expect(result.writes, result.mode).toBe(early ? 0 : 1);
      if (success) expect(result.error, result.mode).toBeNull();
      else
        expect(result.error, result.mode).toBe(
          result.mode === 'error'
            ? 'readback unavailable'
            : result.mode === 'pending'
              ? 'Коллекции ожидают сохранения. Повторите чтение.'
              : 'Добавление требует проверки состава коллекций.',
        );
      if (!early)
        expect(result.requested, result.mode).toEqual({
          dev: 'contract-device',
          expected_revision: 4,
          collections: {
            version: 1,
            revision: 5,
            images: kind === 'image' ? ['image-neighbor', 'new'] : ['image-neighbor'],
            watermarks: ['watermark-neighbor'],
            backgrounds: [
              { kind: 'image', key: 'background-neighbor' },
              { kind: 'video', key: 'video-neighbor' },
              ...(kind === 'video' ? [{ kind: 'video', key: 'new' }] : []),
            ],
          },
        });
    }
  });
