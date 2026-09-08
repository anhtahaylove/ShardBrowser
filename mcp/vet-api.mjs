// Prove the API changes end to end: bienTheCuaKhoa must now return channel_options,
// and the type-selector axes must carry blockedByVariantAxisOfTypes.
import { chromium } from 'patchright'

const b = await chromium.launch({ headless: true })
const page = await b.newPage()
await page.goto('http://127.0.0.1:3000/login', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(1500)
await page.fill('input[type=email], input[name=email]', process.env.SEED_EMAIL)
await page.fill('input[type=password], input[name=password]', process.env.SEED_PASS)
await page.click('button[type=submit]')
await page.waitForTimeout(3500)

const call = async (path, input) => page.evaluate(async ([p, i]) => {
  const u = 'http://127.0.0.1:3001/trpc/' + p + '?input=' + encodeURIComponent(JSON.stringify(i))
  const r = await fetch(u, { credentials: 'include' })
  return { status: r.status, body: (await r.text()).slice(0, 4000) }
}, [path, input])

const a = await call('design.bienTheCuaKhoa',
  { designKey: 'FUEL-8A75840', storeId: 109, channelProductId: '8090178289897' })
console.log('=== bienTheCuaKhoa ===', a.status)
console.log(a.body.slice(0, 1600))

const c = await call('sellingProduct.typeSelectorPlan',
  { storeId: 109, channelProductId: '8090178289897' })
console.log('=== axes ===', c.status)
console.log(c.body.slice(0, 1600))
await b.close()
