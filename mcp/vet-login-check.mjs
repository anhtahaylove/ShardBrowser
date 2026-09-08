// Check saved credentials / autofill on the vetgroup login page.
import { chromium } from 'patchright'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

await page.goto('https://vetgroup.net/login', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(2500)
const info = await page.evaluate(() => {
  const inputs = [...document.querySelectorAll('input')].map((i) => ({
    name: i.name, type: i.type, value: i.value ? '<filled>' : '', placeholder: i.placeholder,
  }))
  return { inputs, cookies: document.cookie.slice(0, 200), url: location.href }
})
console.log(JSON.stringify(info, null, 2))
const cookies = await ctx.cookies('https://vetgroup.net')
console.log('cookie names:', cookies.map((c) => c.name).join(', '))
await browser.close()
