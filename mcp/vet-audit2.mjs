// Audit the /designs and /map-file-nha-in pages live.
import { chromium } from 'patchright'
import fs from 'node:fs'

const CDP = process.env.VET_CDP || 'http://127.0.0.1:60525'
const OUT = 'C:/Users/Administrator/AppData/Local/Temp/vet-audit'
fs.mkdirSync(OUT, { recursive: true })

const browser = await chromium.connectOverCDP(CDP)
const ctx = browser.contexts()[0]
const page = ctx.pages().find((p) => !p.url().startsWith('devtools://')) || await ctx.newPage()

for (const [name, url] of [['designs', 'https://vetgroup.net/designs'], ['mapfile', 'https://vetgroup.net/map-file-nha-in']]) {
  const logs = []
  const failed = []
  const onC = (m) => logs.push(`[${m.type()}] ${m.text()}`.slice(0, 300))
  const onE = (e) => logs.push(`[pageerror] ${String(e).slice(0, 300)}`)
  const onR = (r) => { if (r.status() >= 400) failed.push(`${r.status()} ${r.url().slice(0, 140)}`) }
  page.on('console', onC); page.on('pageerror', onE); page.on('response', onR)
  await page.goto(url, { waitUntil: 'networkidle', timeout: 90000 })
  await page.waitForTimeout(8000)
  const txt = await page.evaluate(() => document.body.innerText)
  fs.writeFileSync(`${OUT}/${name}.txt`, txt)
  const i = txt.indexOf('Đang chạy')
  console.log(`\n########## ${name} ##########`)
  console.log(txt.slice(i + 10, i + 3200))
  console.log('--- console:', JSON.stringify(logs.slice(0, 8)))
  console.log('--- failed:', JSON.stringify(failed.slice(0, 8)))
  page.off('console', onC); page.off('pageerror', onE); page.off('response', onR)
}
await browser.close()
