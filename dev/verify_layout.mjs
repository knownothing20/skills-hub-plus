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

// 页面刚加载完时 React 还没把技能列表画出来（后端 invoke 有延迟），
// 直接测量会得到 0 行、随后 querySelector 取到 null 而抛错。
// 这里先轮询等卡片出现，最多等 30 秒。
let waited = 0
for (;;) {
  const n = await ev(`document.querySelectorAll('.list-view .skill-card').length`)
  if (n > 0) { console.log(`已渲染 ${n} 行（等待 ${waited} 秒）`); break }
  if (waited >= 30) { console.error('等待超时：技能列表始终是空的，请确认应用已加载数据'); process.exit(1) }
  await new Promise(r => setTimeout(r, 1000))
  waited++
}

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

  // 徽标越界检测：必须区分两个方向。
  //   垂直越界 = 真裁切（overflow-x:auto 会把另一轴也算成 auto），这是要修的 bug。
  //   水平越界 = 图标横向滚出可视区，属于正常滚动，不是裁切，不能算缺陷。
  // 早先把两者混在一起报，导致「修好了」之后仍显示 clippedWraps: 7，误判为没修好。
  const clip = []
  document.querySelectorAll('.list-view .skill-tool-avatars').forEach(wrap => {
    const wr = R(wrap)
    const sel = '.version-badge, .stale-badge, .instance-badge, .owner-crown, .sync-mode-badge'
    const bad = Array.from(wrap.querySelectorAll(sel)).filter(b => {
      const r = R(b)
      const vertOut = r.y < wr.y - 1 || r.y + r.h > wr.y + wr.h + 1
      const horizOut = r.x < wr.x - 1 || r.x + r.w > wr.x + wr.w + 1
      return vertOut || horizOut
    })
    if (bad.length) {
      clip.push({
        count: bad.length,
        sample: bad.slice(0, 2).map(b => {
          const r = R(b)
          const vertOut = r.y < wr.y - 1 || r.y + r.h > wr.y + wr.h + 1
          return { cls: b.className, vertOut, horizOut: !vertOut }
        }),
      })
    }
  })
  const vertClippedWraps = clip.filter(c => c.sample.some(s => s.vertOut)).length

  return {
    totalRows: cards.length,
    truncatedRows: rows.filter(r => r.truncated).map(r => ({ name: r.name, w: r.nameW, need: r.need })),
    vertClippedWraps,
    clippedWraps: clip.length,
    clipSample: clip.slice(0, 3),
    sample: rows.slice(0, 7),
  }
})()`)
console.log(JSON.stringify(out, null, 2))

const detail = await ev(`(() => {
  // 有的行可能没有工具图标（wrap 为 null），跳到第一个有图标区的行再测量。
  const wrapEl = Array.from(document.querySelectorAll('.list-view .skill-tool-avatars'))[0]
  if (!wrapEl) return { error: '任何一行都没有工具图标区' }
  const card = wrapEl.closest('.skill-card')
  const wrap = wrapEl
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
