import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertTriangle, CheckCircle2, ChevronDown, ChevronRight } from 'lucide-react'
import { invokeTauri } from '../../hooks/useTauriInvoke'

type PeerStanding = {
  tool: string
  instance: string
  path: string
  state: 'in_sync' | 'edited' | 'missing'
  changed_files: string[]
  file_count: number
}

type Props = {
  skillId: string
}

function labelOf(p: PeerStanding): string {
  return p.instance ? `${p.tool} / ${p.instance}` : p.tool
}

/**
 * 详情页顶部的副本状态条：只在有副本偏离基线时才显眼，
 * 多个版本并存时点开能看到各自改了哪些文件。
 */
export default function PeerStatusBanner({ skillId }: Props) {
  const [peers, setPeers] = useState<PeerStanding[]>([])
  const [loading, setLoading] = useState(true)
  const [expanded, setExpanded] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const list = await invokeTauri<PeerStanding[]>('get_peer_standings', { skillId })
      setPeers(list || [])
    } catch {
      setPeers([])
    } finally {
      setLoading(false)
    }
  }, [skillId])

  useEffect(() => {
    void load()
  }, [load])

  const edited = useMemo(() => peers.filter((p) => p.state === 'edited'), [peers])
  const missing = useMemo(() => peers.filter((p) => p.state === 'missing'), [peers])

  if (loading || peers.length === 0) return null

  // 全部一致：一行淡淡的确认，不打扰
  if (edited.length === 0 && missing.length === 0) {
    return (
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
          padding: '7px 12px',
          marginBottom: '12px',
          fontSize: '11px',
          color: 'var(--text-tertiary)',
          border: '1px solid var(--border-subtle)',
          borderRadius: '8px',
        }}
      >
        <CheckCircle2 size={13} style={{ color: '#059669' }} />
        {peers.length} 个副本内容完全一致
      </div>
    )
  }

  const isConflict = edited.length > 1
  const tone = isConflict
    ? { bg: 'rgba(226,75,74,.06)', border: 'rgba(226,75,74,.45)', color: '#a32d2d' }
    : { bg: 'rgba(245,158,11,.07)', border: 'rgba(245,158,11,.45)', color: '#b45309' }

  return (
    <div
      style={{
        marginBottom: '12px',
        padding: '8px 12px',
        background: tone.bg,
        border: `1px solid ${tone.border}`,
        borderRadius: '8px',
        fontSize: '12px',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        <AlertTriangle size={14} style={{ color: tone.color }} />
        <span style={{ color: tone.color, fontWeight: 500 }}>
          {isConflict
            ? `${edited.length} 个副本改过且内容不同，需要选一个`
            : `${edited.length} 个副本有改动，等其它副本同步`}
        </span>
        {missing.length > 0 && (
          <span style={{ color: 'var(--text-tertiary)', fontSize: '11px' }}>· {missing.length} 个目录缺失</span>
        )}
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          style={{
            marginLeft: 'auto',
            display: 'inline-flex',
            alignItems: 'center',
            gap: '3px',
            background: 'transparent',
            border: 0,
            cursor: 'pointer',
            fontSize: '11px',
            color: 'var(--text-secondary)',
          }}
        >
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
          {expanded ? '收起' : '看是哪几个'}
        </button>
      </div>

      {expanded && (
        <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
          {edited.map((p) => (
            <div
              key={`${p.tool}-${p.instance}-${p.path}`}
              style={{
                padding: '7px 9px',
                background: 'var(--bg-panel)',
                border: '1px solid var(--border-subtle)',
                borderRadius: '6px',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <span style={{ fontSize: '12px', fontWeight: 500 }}>{labelOf(p)}</span>
                <span style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
                  改过 {p.changed_files.length} 个文件 · 共 {p.file_count} 个
                </span>
              </div>
              <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginTop: '3px', lineHeight: 1.6 }}>
                {p.changed_files.slice(0, 6).join('、')}
                {p.changed_files.length > 6 ? ` 等 ${p.changed_files.length} 个` : ''}
              </div>
            </div>
          ))}
          {missing.length > 0 && (
            <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
              目录缺失：{missing.map(labelOf).join('、')}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
