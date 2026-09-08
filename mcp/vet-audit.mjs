// Drive the VN Automation profile over CDP for a vetgroup.net UI audit.
import { chromium } from 'patchright'
import fs from 'node:fs'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const url = process.argv[2]
const outFile = process.argv[3] || null

const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

const logs = []
page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`.slice(0, 400)))
page.on('pageerror', (e) => logs.push(`[pageerror] ${String(e).slice(0, 400)}`))
const failed = []
page.on('response', (r) => { if (r.status() >= 400) failed.push(`${r.status()} ${r.url()}`) })

if (url) {
  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 })
  await page.waitForTimeout(4000)
}

const text = await page.evaluate(() => document.body.innerText)
const out = {
  url: page.url(),
  title: await page.title(),
  text: text.slice(0, 20000),
  console: logs.slice(0, 40),
  failedRequests: failed.slice(0, 20),
}
if (outFile) fs.writeFileSync(outFile, JSON.stringify(out, null, 2))
console.log(JSON.stringify({ url: out.url, title: out.title, console: out.console, failedRequests: out.failedRequests }, null, 2))
console.log('--- TEXT ---')
console.log(out.text.slice(0, 6000))
await browser.close()
