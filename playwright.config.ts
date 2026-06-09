import { defineConfig } from '@playwright/test'

const chromiumExecutablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH

export default defineConfig({
  testDir: './apps/web/e2e',
  timeout: 30_000,
  use: {
    baseURL: 'http://127.0.0.1:4173',
    headless: true,
    ...(chromiumExecutablePath ? { launchOptions: { executablePath: chromiumExecutablePath } } : {}),
  },
  webServer: [
    {
      command: 'cargo run -p octomonitor-server',
      port: 46321,
      reuseExistingServer: true,
      timeout: 120_000,
    },
    {
      command: 'pnpm --filter @octomonitor/web dev --host 127.0.0.1 --port 4173',
      port: 4173,
      reuseExistingServer: true,
      timeout: 120_000,
    },
  ],
})
