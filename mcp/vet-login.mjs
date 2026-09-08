// Log in to vetgroup.net using the profile's saved autofill credentials.
import { chromium } from 'patchright'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

if (!page.url().includes('/login')) {
  await page.goto('https://vetgroup.net/login', { waitUntil: 'domcontentloaded' })
}
await page.waitForTimeout(2000)
await page.click('button[type=submit], button:has-text("Đăng nhập")')
await page.waitForTimeout(6000)
console.log('after submit:', page.url())
console.log((await page.evaluate(() => document.body.innerText)).slice(0, 1200))
await browser.close()
