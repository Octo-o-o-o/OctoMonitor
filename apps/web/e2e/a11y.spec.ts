import { test, expect } from '@playwright/test'
import AxeBuilder from '@axe-core/playwright'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const reportDir = path.resolve(__dirname, '../test-results')
const reportPath = path.resolve(reportDir, 'a11y-audit.json')
const outputPath = path.resolve(reportDir, 'a11y-output.txt')

const tabs = [
  { key: 'monitor', name: 'MONITOR' },
  { key: 'usage', name: 'USAGE' },
  { key: 'commits', name: 'COMMITS' },
  { key: 'heatmap', name: 'HEATMAP' },
  { key: 'settings', name: 'SETTINGS' },
] as const

test('wcag audit across primary tabs', async ({ page, baseURL }) => {
  const details: Array<{ url: string; violations: unknown[] }> = []

  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto(`${baseURL}/`)
  await expect(page.locator('body')).toBeVisible()

  for (const tab of tabs) {
    await page.getByRole('tab', { name: tab.name }).click()
    const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa']).analyze()
    details.push({ url: `/${tab.key}`, violations: results.violations })
  }

  const totalViolations = details.reduce((sum, item) => sum + item.violations.length, 0)
  fs.mkdirSync(reportDir, { recursive: true })
  fs.writeFileSync(reportPath, JSON.stringify({ pages: tabs.map((tab) => `/${tab.key}`), totalViolations, details }, null, 2))
  fs.writeFileSync(outputPath, JSON.stringify({ pages: tabs.length, totalViolations }, null, 2))
  expect(totalViolations).toBe(0)
})
