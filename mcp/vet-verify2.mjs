// Prove the assign-file dialog now offers an input even with no derived zone.
import { chromium } from 'patchright'
import fs from 'node:fs'

const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
const b = await chromium.launch({ headless: true })
const p = await b.newContext({ viewport: { width: 1680, height: 1200 } })
const page = await p.newPage()
const errors = []
page.on('pageerror', (e) => errors.push(String(e).slice(0, 200)))

await page.goto('http://127.0.0.1:3000/', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(2500)
if (page.url().includes('login') || (await page.locator('input[type=password]').count())) {
  await page.locator('input').first().fill(process.env.SEED_EMAIL)
  await page.locator('input[type=password]').first().fill(process.env.SEED_PASS)
  await page.locator('button[type=submit]').first().click()
  await page.waitForTimeout(9000)
}
await page.goto('http://127.0.0.1:3000/products', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(12000)

// Default view lists listings with no type match - the dead-end case.
const rows = page.locator('table tbody tr')
console.log('ROWS', await rows.count())
// The assign-file dialog is reached from the SKU tab, not the listing tab.
await page.getByText('SKU', { exact: true }).first().click().catch(() => {})
await page.waitForTimeout(11000)
const r2 = page.locator('table tbody tr')
console.log('SKU_ROWS', await r2.count())
if (await r2.count()) {
  await r2.nth(0).click().catch(() => {})
  await page.waitForTimeout(8000)
}
console.log('AFTER_LEN', (await page.locator('body').innerText()).length)

// Open the assign-file dialog: that is where the dead end lived.
const btn = page.getByRole('button', { name: /Gán file in|Thêm file in|file in/i })
console.log('ASSIGN_BUTTONS', await btn.count())
if (await btn.count()) { await btn.first().click().catch(() => {}); await page.waitForTimeout(7000) }

const txt = await page.locator('body').innerText()
fs.writeFileSync(OUT + '/verify2.txt', txt)
const boxes = await page.locator('textarea').count()
console.log('TEXTAREAS', boxes)
for (const k of ['File chung', 'chưa khớp loại hàng nào', 'Gán file in', 'Gán chung file in']) {
  console.log((txt.includes(k) ? 'HIT  ' : 'miss ') + k)
}
console.log('ERRORS', JSON.stringify(errors))
await b.close()
