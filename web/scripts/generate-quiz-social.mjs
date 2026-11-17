import { chromium } from '@playwright/test'
import { fileURLToPath } from 'node:url'
const browser = await chromium.launch({ headless: true })
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 627 }, deviceScaleFactor: 1 })
  await page.goto(new URL('../public/brand/cdl-quiz-social.svg', import.meta.url).href)
  await page.screenshot({ path: fileURLToPath(new URL('../public/brand/cdl-quiz-social.png', import.meta.url)), omitBackground: false })
} finally { await browser.close() }
