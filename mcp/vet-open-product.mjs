// Dump products page after full load, then expand the first row.
import { chromium } from 'patchright'
import fs from 'node:fs'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
fs.mkdirSync(OUT, { recursive: true })

const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

const logs = []
page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`.slice(0, 300)))
page.on('pageerror', (e) => logs.push(`[pageerror] ${String(e).slice(0, 300)}`))
const failed = []
page.on('response', (r) => { if (r.status() >= 400) failed.push(`${r.status()} ${r.url().slice(0, 200)}`) })

await page.goto('https://vetgroup.net/products?ban_q=TSHIRT-WC-184-AD-DTG', { waitUntil: 'networkidle', timeout: 90000 })
await page.waitForTimeout(8000)
const txt = await page.evaluate(() => document.body.innerText)
fs.writeFileSync(OUT + '/products-1.txt', txt)
console.log(txt.slice(500, 4500))
console.log('=== CONSOLE ===', JSON.stringify(logs.slice(0, 15), null, 1))
console.log('=== FAILED ===', JSON.stringify(failed.slice(0, 10), null, 1))
await browser.close()
