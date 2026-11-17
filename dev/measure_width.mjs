// 卡片为什么只有 896px：查 list-view 容器与卡片自身的宽度上限。
const port = process.argv[2] ?? '9223'
const t = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
const p = t.find(x => x.type === 'page')
const ws = new WebSocket(p.webSocketDebuggerUrl)
let seq = 0; const pend = new Map()
const send = (m, params = {}) => new Promise((res, rej) => { const id = ++seq; pend.set(id, { res, rej }); ws.send(JSON.stringify({ id, method: m, params })) })
ws.addEventListener('message', e => { const m = JSON.parse(e.data); const s = pend.get(m.id); if (!s) return; pend.delete(m.id); m.error ? s.rej(new Error(m.error.message)) : s.res(m.result) })
await new Promise((r, j) => { ws.addEventListener('open', r); ws.addEventListener('error', j) })
const ev = async (x) => { const r = await send('Runtime.evaluate', { expression: x, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails)); return r.result.value }

for (let i = 0; i < 30; i++) {
  const n = await ev(`document.querySelectorAll('.list-view .skill-card').length`)
  if (n > 0) break
  await new Promise(r => setTimeout(r, 1000))
}

const out = await ev(`(() => {
  const card = document.querySelector('.list-view .skill-card')
  const W = el => el ? Math.round(el.getBoundingClientRect().width) : null
  const chain = []
  let el = card
  while (el && el !== document.documentElement) {
    const cs = getComputedStyle(el)
    chain.push({
      tag: el.tagName.toLowerCase(),
      cls: (el.className || '').toString().slice(0, 60),
      w: W(el),
      maxW: cs.maxWidth, w2: cs.width, flex: cs.flex,
      padL: cs.paddingLeft, padR: cs.paddingRight,
      display: cs.display,
    })
    el = el.parentElement
  }
  return {
    winInner: window.innerWidth,
    chain,
    // 工具区若要放下 8 个 24px 图标（gap 5）需要多少
    need8: 8 * 24 + 7 * 5,
  }
})()`)
console.log(JSON.stringify(out, null, 2))
ws.close()