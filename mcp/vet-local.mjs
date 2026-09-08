// Log into the local stack and open the products mapping panel.
import { chromium } from 'patchright'
import fs from 'node:fs'

const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
fs.mkdirSync(OUT, { recursive: true })

const b = await chromium.launch({ headless: true })
const page = await b.newPage()
const errs = []
page.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()) })
page.on('pageerror', (e) => { errs.push('PAGEERROR ' + e.message) })

await page.goto('http://127.0.0.1:3000/login', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(2000)
const email = process.env.SEED_EMAIL
const pass = process.env.SEED_PASS
await page.fill('input[type=email], input[name=email]', email)
await page.fill('input[type=password], input[name=password]', pass)
await page.click('button[type=submit]')
await page.waitForTimeout(4000)
console.log('after login:', page.url())

await page.goto('http://127.0.0.1:3000/products', { waitUntil: 'domcontentloaded' })
await page.waitForTimeout(6000)
const txt = await page.evaluate(() => document.body.innerText)
fs.writeFileSync(OUT + '/local-products.txt', txt, 'utf8')
console.log('LEN', txt.length)
console.log('ERRORS', JSON.stringify(errs.slice(0, 10), null, 1))
await b.close()
