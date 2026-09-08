// Prove the missing-zone dead end is gone: an unmatched listing must still
// offer a place to type a design link.
import { chromium } from 'patchright'
import fs from 'node:fs'

const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
fs.mkdirSync(OUT, { recursive: true })

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

// Default filter shows listings that are NOT matched yet - exactly the case
// that used to render nothing.
const rows = page.locator('table tbody tr')
const n = await rows.count()
console.log('ROWS', n)
if (n > 0) {
  const caret = rows.nth(0).locator('button').first()
  if (await caret.count()) await caret.click().catch(() => {})
  else await rows.nth(0).click().catch(() => {})
  await page.waitForTimeout(9000)
}

// Open every collapsed design-link panel on screen.
for (const label of ['Link riêng theo từng biến thể', 'Thêm file in', 'Link thiết kế']) {
  const els = page.getByText(label, { exact: false })
  const c = await els.count()
  for (let i = 0; i < Math.min(c, 3); i++) {
    await els.nth(i).click().catch(() => {})
    await page.waitForTimeout(5000)
  }
}

const txt = await page.locator('body').innerText()
fs.writeFileSync(OUT + '/verify.txt', txt)
for (const k of ['Link riêng theo từng biến thể', 'chưa rõ vùng', 'Gom theo', 'Nhóm biến thể',
                 'áp cho cả', 'chưa khớp loại hàng nào', 'đang sinh biến thể']) {
  console.log((txt.includes(k) ? 'HIT  ' : 'miss ') + k)
}
console.log('LEN', txt.length)
console.log('ERRORS', JSON.stringify(errors))
await b.close()
