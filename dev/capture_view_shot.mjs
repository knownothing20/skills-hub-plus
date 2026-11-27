// 切换视图模式后截图（WebView2 CDP 直连，无需装 playwright）。
//
// 用法：
//   1. 带调试端口启动应用：
//      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223" \
//        ./dist-app/skills-hub.exe
//   2. 抓图：
//      node dev/capture_view_shot.mjs <port> <输出png> <list|cards> [scale]
//
// 视图模式存于 localStorage['skills-view-mode']，合法值是 'list' | 'cards'
// （不是 'card'）。改完必须 reload 才会生效。
//
// scale 默认 2：docs/assets 里的现有素材是 2360x1520（1180x760 的 2 倍），
// 抓 1x 会和其它截图对不上清晰度。
const port = process.argv[2] ?? '9223'
const out = process.argv[3] ?? 'app-shot.png'
const view = process.argv[4] ?? 'cards'
const scale = Number(process.argv[5] ?? '2')

if (!['list', 'cards'].includes(view)) {
  console.error(`视图模式只能是 list 或 cards，收到: ${view}`)
  process.exit(1)
}

const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json()
const page = targets.find((t) => t.type === 'page' && t.webSocketDebuggerUrl)
if (!page) {
  console.error(`没找到可调试的页面目标（端口 ${port}）：`, targets.map((t) => t.type))
  process.exit(1)
}

const ws = new WebSocket(page.webSocketDebuggerUrl)
let seq = 0
const pending = new Map()

const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++seq
    pending.set(id, { resolve, reject })
    ws.send(JSON.stringify({ id, method, params }))
  })

ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data)
  const slot = pending.get(msg.id)
  if (!slot) return
  pending.delete(msg.id)
  if (msg.error) slot.reject(new Error(msg.error.message))
  else slot.resolve(msg.result)
})

await new Promise((resolve, reject) => {
  ws.addEventListener('open', resolve, { once: true })
  ws.addEventListener('error', () => reject(new Error('CDP 连接失败')), { once: true })
})

await send('Page.enable')
await send('Runtime.enable')

// 写 localStorage 并 reload。reload 会断开本次导航的页面上下文，
// 所以先设值、再单独发 reload，然后等 load 事件。
await send('Runtime.evaluate', {
  expression: `localStorage.setItem('skills-view-mode', ${JSON.stringify(view)}); true`,
})

await send('Page.reload', { ignoreCache: false })
// 等骨架渲染 + 技能列表回填
await new Promise((r) => setTimeout(r, 4000))

// 复核实际生效的视图模式，防止写到错的键/值上却以为切成功了
const check = await send('Runtime.evaluate', {
  expression: `localStorage.getItem('skills-view-mode')`,
  returnByValue: true,
})
console.log(`视图模式: ${check.result?.value}`)

// 2x 截图要改 devicePixelRatio 让它真正重新栅格化；
// Page.captureScreenshot 的 clip.scale 只放大像素，文字会糊。
//
// 视口固定 1180x760（不跟随窗口实际大小）：docs/assets 现有素材都是
// 2360x1520，尺寸必须一致，否则 README 里几张图并排会高低不齐。
const metrics = await send('Runtime.evaluate', {
  expression: `({ w: window.innerWidth, h: window.innerHeight })`,
  returnByValue: true,
})
console.log(`实际窗口: ${metrics.result.value.w}x${metrics.result.value.h}`)
await send('Emulation.setDeviceMetricsOverride', {
  width: 1180,
  height: 760,
  deviceScaleFactor: scale,
  mobile: false,
})
await new Promise((r) => setTimeout(r, 1500))

const { data } = await send('Page.captureScreenshot', { format: 'png' })
const { writeFileSync } = await import('node:fs')
writeFileSync(out, Buffer.from(data, 'base64'))
console.log(`已保存截图: ${out}（${view}）`)
ws.close()