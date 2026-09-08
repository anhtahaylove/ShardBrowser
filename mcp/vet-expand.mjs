// Expand the product row, then the variant-level design link table.
import { chromium } from 'patchright'
import fs from 'node:fs'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
fs.mkdirSync(OUT, { recursive: true })

const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

const logs = []
page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`.slice(0, 400)))
page.on('pageerror', (e) => logs.push(`[pageerror] ${String(e).slice(0, 400)}`))
const failed = []
page.on('response', (r) => { if (r.status() >= 400) failed.push(`${r.status()} ${r.url().slice(0, 160)}`) })

await page.goto('https://vetgroup.net/products?ban_q=TSHIRT-WC-184-AD-DTG', { waitUntil: 'networkidle', timeout: 90000 })
await page.waitForTimeout(7000)

// 1. Expand the listing row (click the title cell).
await page.evaluate(() => {
  const row = [...document.querySelectorAll('tbody tr')].find((r) => (r.textContent || '').includes('Every Child Matters'))
  if (row) row.click()
})
await page.waitForTimeout(9000)
let txt = await page.evaluate(() => document.body.innerText)
console.log('after row click, has detail:', txt.includes('Link thiết kế theo từng vùng in'))

// 2. Expand the two nested toggles.
const clicked = await page.evaluate(() => {
  const want = ['Link riêng theo từng biến thể', 'Xem file hệ thống sẽ dùng']
  const hits = []
  for (const el of document.querySelectorAll('button, summary, [role=button], a')) {
    const t = (el.textContent || '').trim()
    if (want.some((x) => t.includes(x)) && t.length < 80) { el.click(); hits.push(t) }
  }
  return hits
})
console.log('CLICKED:', JSON.stringify(clicked))
await page.waitForTimeout(9000)

txt = await page.evaluate(() => document.body.innerText)
fs.writeFileSync(OUT + '/products-expanded.txt', txt)
const i = txt.indexOf('Link riêng theo từng biến thể')
console.log('--- SLICE ---')
console.log(txt.slice(i, i + 3500))
console.log('=== CONSOLE ===', JSON.stringify(logs.slice(0, 20), null, 1))
console.log('=== FAILED ===', JSON.stringify(failed.slice(0, 12), null, 1))
await browser.close()
