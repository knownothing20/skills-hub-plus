// 一次性验证：注入「identity-copy 撑满 + 候选列宽」，看名称是否还截断。
// 用来在出包前确认 CSS 修复有效，避免又白等一轮云端构建。
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

const cases = [
  ['240px（旧上限）+ copy 修复', 'minmax(200px, 240px) fit-content(150px) 46px 64px minmax(200px, 1fr) 42px 110px 16px'],
  ['256px（新上限）+ copy 修复', 'minmax(200px, 256px) fit-content(150px) 46px 64px minmax(184px, 1fr) 42px 110px 16px'],
]

for (const [label, cols] of cases) {
  const out = await ev(`(() => {
    document.getElementById('__probe')?.remove()
    const st = document.createElement('style')
    st.id = '__probe'
    st.textContent = \`
      .list-view .skill-card { grid-template-columns: ${cols} !important; }
      .skill-identity-copy { flex: 1 1 auto !important; min-width: 0 !important; }
    \`
    document.head.appendChild(st)

    const cards = Array.from(document.querySelectorAll('.list-view .skill-card'))
    const res = cards.map(card => {
      const nameEl = card.querySelector('.skill-name')
      if (!nameEl) return null
      const s = document.createElement('span')
      s.style.cssText = 'position:absolute;visibility:hidden;white-space:nowrap;font:' + getComputedStyle(nameEl).font
      s.textContent = nameEl.textContent
      document.body.appendChild(s)
      const need = Math.round(s.getBoundingClientRect().width)
      s.remove()
      const w = Math.round(nameEl.getBoundingClientRect().width)
      return { name: nameEl.textContent, w, need, truncated: w < need - 2 }
    }).filter(Boolean)

    const first = cards[0]
    return {
      nameW: Math.round(first.querySelector('.skill-name').getBoundingClientRect().width),
      copyW: Math.round(first.querySelector('.skill-identity-copy').getBoundingClientRect().width),
      titleW: Math.round(first.querySelector('.skill-title-line').getBoundingClientRect().width),
      truncated: res.filter(r => r.truncated),
    }
  })()`)
  console.log(`\n=== ${label} ===`)
  console.log(JSON.stringify(out, null, 2))
}

await ev(`document.getElementById('__probe')?.remove()`)
ws.close()