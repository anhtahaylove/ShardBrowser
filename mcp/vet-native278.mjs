// Open the selling-products tab, search the SKU, expand the listing row.
import { chromium } from 'patchright'
import fs from 'node:fs'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
fs.mkdirSync(OUT, { recursive: true })

const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

const logs = []
page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`.slice(0, 200)))
page.on('pageerror', (e) => logs.push(`[pageerror] ${String(e).slice(0, 200)}`))

await page.goto('https://vetgroup.net/products?tab=ban', { waitUntil: 'networkidle', timeout: 90000 })
await page.waitForTimeout(5000)

const box = page.locator('main input:not([readonly])').first()
await box.fill('NATIVE-TSHIRT-278-DTG')
await page.waitForTimeout(6000)

const rows = page.locator('table tbody tr')
console.log('rows:', await rows.count())
await rows.first().click()
await page.waitForTimeout(12000)

const txt = await page.evaluate(() => document.body.innerText)
fs.writeFileSync(`${OUT}/native278-detail.txt`, txt)
const i = txt.indexOf('Thuộc tính nào')
console.log('idx type-selector:', i, 'len', txt.length)
console.log(txt.slice(Math.max(0, i - 800), i + 6000))
console.log('=== console:', JSON.stringify(logs.slice(0, 10)))
await browser.close()
