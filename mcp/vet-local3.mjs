// Verify the grouped variant design-link UI against the seeded local listing.
import { chromium } from 'patchright'
import fs from 'node:fs'

const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
const b = await chromium.launch({ headless: true })
const page = await b.newPage()
const errs = []
page.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()) })
page.on('pageerror', (e) => { errs.push('PAGEERROR ' + e.message) })

await page.goto('http://127.0.0.1:3000/login', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(1500)
await page.fill('input[type=email], input[name=email]', process.env.SEED_EMAIL)
await page.fill('input[type=password], input[name=password]', process.env.SEED_PASS)
await page.click('button[type=submit]')
await page.waitForTimeout(3500)

await page.goto('http://127.0.0.1:3000/products', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(5000)

// Reach the SKU panel through the listing row that actually has a supplier route:
// open the listing, then its SKU block, where the design-link table lives.
// The listing is fully matched now, so the default "still stuck" view hides it.
// The SKU table is already on screen; the seeded SKU sits behind the
// "chưa có file in" counter of its product type.
for (const t of ['TSHIRT-DTG', 'chưa có file in']) {
  const el = page.getByText(t, { exact: false }).first()
  if (await el.count()) { await el.click().catch(() => {}); await page.waitForTimeout(6000) }
}
// Clicking the type filter collapses the per-SKU table; bring it back.
const show = page.getByText('bảng theo SKU', { exact: false }).first()
if (await show.count()) { await show.click().catch(() => {}); await page.waitForTimeout(7000) }
const rows2 = page.locator('table tbody tr')
const n2 = await rows2.count()
let hit = -1
for (let i = 0; i < n2; i++) {
  const t = await rows2.nth(i).innerText().catch(() => '')
  if (t.includes('FUEL-8A75840')) { hit = i; break }
}
console.log('SKU_ROWS', n2, 'HIT', hit)
if (hit >= 0) {
  const caret = rows2.nth(hit).locator('button').first()
  if (await caret.count()) await caret.click().catch(() => {})
  await page.waitForTimeout(9000)
}
console.log('AFTER_EXPAND', (await page.locator('body').innerText()).length)

const txt = await page.evaluate(() => document.body.innerText)
fs.writeFileSync(OUT + '/local-grouped.txt', txt, 'utf8')
console.log('LEN', txt.length)
for (const k of ['Link riêng theo màu / loại / biến thể', 'Gom theo', 'Color', 'Size',
                 'từng biến thể riêng', 'áp cho cả', 'biến thể', 'front', 'back',
                 'chưa khớp loại hàng nào', 'khác nhau giữa']) {
  console.log((txt.includes(k) ? 'HIT  ' : 'miss ') + k)
}
console.log('ERRORS', JSON.stringify(errs.slice(0, 6)))
await b.close()
