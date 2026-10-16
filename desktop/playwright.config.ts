// Existing workflow assertions explicitly choose Russian; English default and
// cache-policy tests clear/replace this saved preference in their own fixtures.
import { defineConfig } from '@playwright/test';
export default defineConfig({ testDir: './tests', use: { baseURL: 'http://127.0.0.1:1420', headless: true, storageState: {cookies: [], origins: [{origin:'http://127.0.0.1:1420',localStorage:[{name:'lconnect.interface-language',value:'ru-RU'}]}]} }, webServer: { command: 'pnpm dev', url: 'http://127.0.0.1:1420', reuseExistingServer: false }, workers: 1 });
