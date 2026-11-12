// 抓取正在运行的 Skills Hub 窗口截图（WebView2 CDP 直连，无需装 playwright）。
//
// 用法：
//   1. 带调试端口启动应用：
//      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223" \
//        ./dist-app/skills-hub.exe
//   2. 抓图：
//      node dev/capture_app_shot.mjs [port] [输出png]
//
// 依赖：Node 18+（用到内置 fetch 与 WebSocket），无第三方包。
const port = process.argv[2] ?? '9223'
const out = process.argv[3] ?? 'app-shot.png'

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
await send('Page.bringToFront')
// 等一帧渲染完成再抓
await new Promise((r) => setTimeout(r, 800))

const { data } = await send('Page.captureScreenshot', { format: 'png' })
const { writeFileSync } = await import('node:fs')
writeFileSync(out, Buffer.from(data, 'base64'))
console.log(`已保存截图: ${out}（${page.title || page.url}）`)
ws.close()
