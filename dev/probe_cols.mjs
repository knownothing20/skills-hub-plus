// 不改源码、不重新出包：直接把候选列宽注入运行中的页面，重测名称是否还截断。
// 用来在等云端构建之前先确认列宽方案的数值是否成立。
const port = process.argv[2] ?? '9223'
const cols = process.argv[3] ?? 'minmax(200px, 256px) fit-content(150px) 46px 64px minmax(184px, 1fr) 42px 110px 16px'
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
  // 用内联样式覆盖网格（优先级高于样式表，等价于「新列宽生效」），
  // 注入的 <style> 便于之后统一移除。
  const st = document.createElement('style')
  st.id = '__probe_cols'
  st.textContent = '.list-view .skill-card { grid-template-columns: ${cols} !important; }'
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

  const wrap = document.querySelector('.list-view .skill-tool-avatars')
  const result = {
    appliedCols: getComputedStyle(cards[0]).gridTemplateColumns,
    nameColW: Math.round(cards[0].querySelector('.skill-identity').getBoundingClientRect().width),
    truncated: res.filter(r => r.truncated),
    toolBlockW: wrap ? Math.round(wrap.closest('.skill-tools-block').getBoundingClientRect().width) : null,
    total: res.length,
  }
  st.remove()
  return result
})()`)
console.log(JSON.stringify(out, null, 2))
ws.close()