// 出包后实测：技能名是否还截断、工具区徽标是否还被裁切。
// 用法: node dev/verify_layout.mjs <port>
const port = process.argv[2] ?? '9223'
const t = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
const p = t.find(x => x.type === 'page')
if (!p) { console.error('无页面'); process.exit(1) }
const ws = new WebSocket(p.webSocketDebuggerUrl)
let seq = 0; const pend = new Map()
const send = (m, params = {}) => new Promise((res, rej) => { const id = ++seq; pend.set(id, { res, rej }); ws.send(JSON.stringify({ id, method: m, params })) })
ws.addEventListener('message', e => { const m = JSON.parse(e.data); const s = pend.get(m.id); if (!s) return; pend.delete(m.id); m.error ? s.rej(new Error(m.error.message)) : s.res(m.result) })
await new Promise((r, j) => { ws.addEventListener('open', r); ws.addEventListener('error', j) })
const ev = async (x) => { const r = await send('Runtime.evaluate', { expression: x, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails)); return r.result.value }

const out = await ev(`(() => {
  const R = el => { const b = el.getBoundingClientRect(); return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) } }
  const cards = Array.from(document.querySelectorAll('.list-view .skill-card'))
  const rows = cards.map(card => {
    const nameEl = card.querySelector('.skill-name')
    let need = null
    if (nameEl) {
      const s = document.createElement('span')
      s.style.cssText = 'position:absolute;visibility:hidden;white-space:nowrap;font:' + getComputedStyle(nameEl).font
      s.textContent = nameEl.textContent
      document.body.appendChild(s); need = Math.round(s.getBoundingClientRect().width); s.remove()
    }
    const nr = nameEl ? R(nameEl) : null
    const wrap = card.querySelector('.skill-tool-avatars')
    return {
      name: nameEl?.textContent,
      nameW: nr?.w, need,
      truncated: nr && need ? nr.w < need - 2 : null,
      tools: wrap ? wrap.querySelectorAll('button').length : 0,
      toolOverflow: wrap ? wrap.scrollWidth - wrap.clientWidth : 0,
    }
  })

  // 徽标越界检测（含水平与垂直）
  const clip = []
  document.querySelectorAll('.list-view .skill-tool-avatars').forEach(wrap => {
    const wr = R(wrap)
    const sel = '.version-badge, .stale-badge, .instance-badge, .owner-crown, .sync-mode-badge'
    const bad = Array.from(wrap.querySelectorAll(sel)).filter(b => {
      const r = R(b)
      return r.y < wr.y - 1 || r.y + r.h > wr.y + wr.h + 1 || r.x < wr.x - 1 || r.x + r.w > wr.x + wr.w + 1
    })
    if (bad.length) clip.push({ count: bad.length, sample: bad.slice(0,2).map(b => b.className) })
  })

  return {
    totalRows: cards.length,
    truncatedRows: rows.filter(r => r.truncated).map(r => ({ name: r.name, w: r.nameW, need: r.need })),
    clippedWraps: clip.length,
    clipSample: clip.slice(0, 3),
    sample: rows.slice(0, 7),
  }
})()`)
console.log(JSON.stringify(out, null, 2))

const detail = await ev(`(() => {
  const card = document.querySelector('.list-view .skill-card')
  const wrap = card.querySelector('.skill-tool-avatars')
  const blk = card.querySelector('.skill-tools-block')
  const cs = getComputedStyle(wrap)
  const btns = Array.from(wrap.querySelectorAll('button'))
  return {
    blockW: Math.round(blk.getBoundingClientRect().width),
    wrapW: Math.round(wrap.getBoundingClientRect().width),
    wrapClient: wrap.clientWidth, wrapScroll: wrap.scrollWidth,
    gap: cs.gap, pad: cs.padding, overflow: cs.overflowX,
    btnWidths: btns.map(b => Math.round(b.getBoundingClientRect().width)),
    grid: getComputedStyle(card).gridTemplateColumns,
  }
})()`)

const meta = await ev(`(() => {
  const link = Array.from(document.querySelectorAll('link[rel=stylesheet]')).map(l => l.href)
  const probe = document.createElement('div')
  probe.className = 'skill-tool-avatars'
  const btn = document.createElement('button')
  probe.appendChild(btn)
  document.body.appendChild(probe)
  const w = Math.round(btn.getBoundingClientRect().width)
  const gap = getComputedStyle(probe).gap
  probe.remove()
  return { cssLinks: link, probeBtnWidth: w, probeGap: gap, versionLine: document.body.innerText.split('\\n').slice(0,2) }
})()`)
console.log('--- 运行版本/样式来源 ---')
console.log(JSON.stringify(meta, null, 2))

console.log('--- 工具区细节 ---')
console.log(JSON.stringify(detail, null, 2))

ws.close()
