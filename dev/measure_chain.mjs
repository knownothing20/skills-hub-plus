// 名称被截断到底卡在哪一层：逐层测量 identity -> identity-copy -> title-line -> name，
// 并把每层的关键样式打出来，避免再靠猜。
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
  const card = Array.from(document.querySelectorAll('.list-view .skill-card'))
    .find(c => (c.querySelector('.skill-name')?.textContent || '').includes('react-best-practices'))
    || document.querySelector('.list-view .skill-card')
  const W = el => el ? Math.round(el.getBoundingClientRect().width) : null
  const chain = []
  let el = card.querySelector('.skill-name')
  while (el && el !== card) {
    const cs = getComputedStyle(el)
    chain.push({
      cls: el.className,
      w: W(el),
      flex: cs.flex, minW: cs.minWidth, maxW: cs.maxWidth,
      display: cs.display, overflow: cs.overflow, whiteSpace: cs.whiteSpace,
      gridArea: cs.gridArea,
    })
    el = el.parentElement
  }
  const nameEl = card.querySelector('.skill-name')
  return {
    name: nameEl.textContent,
    nameW: W(nameEl),
    identityColW: W(card.querySelector('.skill-identity')),
    chain,
    // 该行工具区/其他列实际占用
    siblings: Array.from(card.children).map(ch => ({
      cls: ch.className,
      w: W(ch),
      area: getComputedStyle(ch).gridArea,
    })),
  }
})()`)
console.log(JSON.stringify(out, null, 2))
ws.close()