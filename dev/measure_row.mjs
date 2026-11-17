// 实测整行宽度预算：卡片总宽、各列实宽、窗口宽度，判断还有多少可争取空间。
const port = process.argv[2] ?? '9223'
const t = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
const p = t.find(x => x.type === 'page')
const ws = new WebSocket(p.webSocketDebuggerUrl)
let seq = 0; const pend = new Map()
const send = (m, params = {}) => new Promise((res, rej) => { const id = ++seq; pend.set(id, { res, rej }); ws.send(JSON.stringify({ id, method: m, params })) })
ws.addEventListener('message', e => { const m = JSON.parse(e.data); const s = pend.get(m.id); if (!s) return; pend.delete(m.id); m.error ? s.rej(new Error(m.error.message)) : s.res(m.result) })
await new Promise((r, j) => { ws.addEventListener('open', r); ws.addEventListener('error', j) })
const ev = async (x) => { const r = await send('Runtime.evaluate', { expression: x, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails)); return r.result.value }

// 等 React 把列表画出来，否则 getComputedStyle 拿到 null 直接抛错。
for (let i = 0; i < 30; i++) {
  const n = await ev(`document.querySelectorAll('.list-view .skill-card').length`)
  if (n > 0) break
  await new Promise(r => setTimeout(r, 1000))
}

const out = await ev(`(() => {
  const card = document.querySelector('.list-view .skill-card')
  const R = el => { if (!el) return null; const b = el.getBoundingClientRect(); return { w: Math.round(b.width), x: Math.round(b.x), h: Math.round(b.height) } }
  const cs = getComputedStyle(card)
  const wrap = card.querySelector('.skill-tool-avatars')
  const blk = card.querySelector('.skill-tools-block')
  const blkCs = getComputedStyle(blk)
  return {
    winInner: window.innerWidth,
    winOuter: window.outerWidth,
    cardW: R(card).w,
    cardPad: cs.paddingLeft + ' / ' + cs.paddingRight,
    cardGap: cs.columnGap,
    gridCols: cs.gridTemplateColumns,
    identity: R(card.querySelector('.skill-identity')),
    meta: R(card.querySelector('.skill-card-meta')),
    origin: R(card.querySelector('.skill-origin-cell')),
    time: R(card.querySelector('.skill-updated')),
    block: R(blk),
    blockPad: blkCs.paddingLeft + ' / ' + blkCs.paddingRight,
    blockGap: blkCs.gap,
    toggle: R(card.querySelector('.skill-switch')),
    actions: R(card.querySelector('.skill-actions') || card.querySelector('.skill-card-footer')),
    arrow: R(card.querySelector('.skill-open-hint')),
    iconNeed: 8 * 24 + 7 * 5,
  }
})()`)
console.log(JSON.stringify(out, null, 2))
ws.close()