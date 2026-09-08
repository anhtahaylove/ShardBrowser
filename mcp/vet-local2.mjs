// Verify the new grouped variant design-link UI on the local stack.
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

// Expand the first listing row that has variants.
const rows = page.locator('table tbody tr')
const n = await rows.count()
console.log('ROWS', n)
let opened = 0
for (let i = 0; i < Math.min(n, 6); i++) {
  await rows.nth(i).click().catch(() => {})
  await page.waitForTimeout(2500)
  const t = await page.evaluate(() => document.body.innerText)
  if (t.includes('Link riêng theo màu') || t.includes('Thuộc tính nào chỉ ra')) { opened = i + 1; break }
}
console.log('OPENED_ROW', opened)

const txt = await page.evaluate(() => document.body.innerText)
fs.writeFileSync(OUT + '/local-mapping.txt', txt, 'utf8')

for (const k of ['Link riêng theo màu / loại / biến thể', 'Gom theo', 'từng biến thể riêng',
                 'Thuộc tính nào chỉ ra', 'trục sinh biến thể', 'mỗi biến thể một link',
                 'áp cho cả', 'chưa khớp loại hàng nào']) {
  console.log((txt.includes(k) ? 'HIT  ' : 'miss ') + k)
}
console.log('ERRORS', JSON.stringify(errs.slice(0, 8)))
await b.close()
