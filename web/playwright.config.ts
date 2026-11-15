import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: './tests/browser',
  testMatch: '**/*.spec.ts',
  workers: 1,
  timeout: 90_000,
  use: { baseURL: 'http://127.0.0.1:4183', headless: true,
    launchOptions: {args:['--disable-background-timer-throttling','--disable-renderer-backgrounding']} },
  webServer: { command: 'npm run dev -- --host 127.0.0.1 --port 4183', url:'http://127.0.0.1:4183', reuseExistingServer:false },
})
