import { memo, useCallback, useEffect, useState } from 'react'
import { RefreshCw, X } from 'lucide-react'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import type { ManagedSkill } from '../types'

export type SkillTargetComparisonDto = {
  tool: string
  tool_label: string
  target_path: string
  central_path: string
  central_mtime: number
  tool_mtime: number
  central_file_count: number
  tool_file_count: number
  status: 'synced' | 'tool_newer' | 'central_newer' | 'missing'
  diff_description: string
  mode: string
  role: string
  instance: string
}

type SyncCompareModalProps = {
  open: boolean
  skill: ManagedSkill
  invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>
  onClose: () => void
  onSuccess?: () => void
  t: TFunction
}

function formatTimestamp(ms: number): string {
  if (!ms || ms <= 0) return '无'
  const d = new Date(ms)
  const pad = (n: number) => n.toString().padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

export const SyncCompareModal = memo(({
  open,
  skill,
  invokeTauri,
  onClose,
  onSuccess,
}: SyncCompareModalProps) => {
  const [loading, setLoading] = useState(true)
  const [operatingTool, setOperatingTool] = useState<string | null>(null)
  const [comparisons, setComparisons] = useState<SkillTargetComparisonDto[]>([])

  const loadData = useCallback(async () => {
    setLoading(true)
    try {
      const res = await invokeTauri<SkillTargetComparisonDto[]>('get_skill_target_comparisons', {
        skillId: skill.id,
      })
      setComparisons(res || [])
    } catch (err) {
      toast.error(`获取比对失败: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      setLoading(false)
    }
  }, [invokeTauri, skill.id])

  useEffect(() => {
    if (open) {
      void loadData()
    }
  }, [open, loadData])

  const handlePromoteToCentral = async (tool: string, toolLabel: string) => {
    if (!window.confirm(`确定要将【${toolLabel}】中的修改反向更新为【全局母版】吗？\n\n这将会用该软件的最新代码更新中央母版。`)) {
      return
    }
    setOperatingTool(tool)
    try {
      await invokeTauri('promote_target_to_central', {
        skillId: skill.id,
        tool,
      })
      toast.success(`已成功将【${toolLabel}】的代码设为全局母版！`)
      await loadData()
      onSuccess?.()
    } catch (err) {
      toast.error(`设为母版失败: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      setOperatingTool(null)
    }
  }

  const handleSyncDown = async (tool: string, toolLabel: string) => {
    if (
      !window.confirm(
        `确定要用【母版最新代码】更新【${toolLabel}】吗？\n\n注意：如果该软件中有未设为母版的私有修改，将会被母版代码替换。`,
      )
    ) {
      return
    }
    setOperatingTool(tool)
    try {
      await invokeTauri('sync_skill_to_tool', {
        // 同步内容取「原件」：母库只是镜像，可能落后于原件
        sourcePath: skill.source_ref || skill.central_path,
        skillId: skill.id,
        tool,
        name: skill.name,
        overwrite: true,
        scope: 'global',
      })
      toast.success(`已成功将母版最新代码更新至【${toolLabel}】！`)
      await loadData()
      onSuccess?.()
    } catch (err) {
      toast.error(`更新此软件失败: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      setOperatingTool(null)
    }
  }

  // 以某个副本为准：把它推给所有其它工具（含原件）
  const handleAdoptAsLatest = async (comp: SkillTargetComparisonDto) => {
    const label = comp.instance ? `${comp.tool_label} · ${comp.instance}` : comp.tool_label
    if (
      !window.confirm(
        `确定把【${label}】这份作为最新版吗？\n\n它会被复制到其它所有工具目录（包括原件），其它副本里的不同内容会被覆盖。`,
      )
    ) {
      return
    }
    setOperatingTool(comp.tool)
    try {
      const updated = await invokeTauri<number>('adopt_target_as_latest', {
        skillId: skill.id,
        tool: comp.tool,
        instance: comp.instance || null,
      })
      toast.success(`已用【${label}】作为最新版，更新了 ${updated} 个副本`)
      await loadData()
      onSuccess?.()
    } catch (err) {
      toast.error(`操作失败: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      setOperatingTool(null)
    }
  }

  if (!open) return null

  const ownerComp = comparisons.find((c) => c.role === 'owner')
  const linkComps = comparisons.filter(
    (c) => c.role === 'link_to_owner' || c.role === 'link_to_central',
  )
  const copyComps = comparisons.filter((c) => c.role === 'copy')
  const brokenComps = comparisons.filter(
    (c) => c.role === 'missing' || c.role === 'link_other',
  )
  // 待处理的排在前面：异常 > 未同步 > 已一致
  const severity = (c: SkillTargetComparisonDto) =>
    c.role === 'missing' || c.role === 'link_other'
      ? 0
      : c.status === 'synced'
        ? 2
        : 1
  const copyRows = [...copyComps].sort((a, b) => severity(a) - severity(b))
  const linkSummary = Object.values(
    linkComps.reduce<Record<string, { label: string; count: number }>>((acc, c) => {
      const cur = acc[c.tool] ?? { label: c.tool_label, count: 0 }
      cur.count += 1
      acc[c.tool] = cur
      return acc
    }, {}),
  )
  const ownerFiles = ownerComp?.tool_file_count ?? comparisons[0]?.central_file_count ?? 0
  const ownerMtime = ownerComp?.tool_mtime ?? comparisons[0]?.central_mtime ?? 0

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal modal-lg"
        onClick={(e) => e.stopPropagation()}
        style={{
          maxWidth: '820px',
          width: '94%',
          maxHeight: '88vh',
          display: 'flex',
          flexDirection: 'column',
          padding: 0,
          overflow: 'hidden',
          borderRadius: '12px',
          boxShadow: '0 20px 40px rgba(0,0,0,0.3)',
        }}
      >
        <div
          className="modal-header"
          style={{
            padding: '16px 20px',
            borderBottom: '1px solid var(--color-border, #e5e7eb)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <RefreshCw size={18} style={{ color: 'var(--color-primary, #2563eb)' }} />
            <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>
              多端对比与分发链路 - {skill.name}
            </h3>
          </div>
          <button
            type="button"
            className="btn-icon"
            onClick={onClose}
            style={{ border: 'none', background: 'transparent', cursor: 'pointer', padding: '4px' }}
          >
            <X size={18} />
          </button>
        </div>

        <div
          className="modal-body"
          style={{
            padding: '20px',
            overflowY: 'auto',
            display: 'flex',
            flexDirection: 'column',
            gap: '16px',
          }}
        >
          {loading && (
            <div style={{ fontSize: '11px', color: '#9ca3af' }}>正在扫描各软件目录并比对…</div>
          )}

          {/* 原件 */}
          <div
            style={{
              padding: '13px 15px',
              backgroundColor: 'rgba(37, 99, 235, 0.06)',
              border: '1px solid rgba(37, 99, 235, 0.18)',
              borderRadius: '8px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
                <span style={{ width: '8px', height: '8px', borderRadius: '50%', background: '#3b82f6', display: 'inline-block' }} />
                <span style={{ fontSize: '13px', fontWeight: 600, color: '#1d4ed8' }}>主 skill（最初创建位置）</span>
                {ownerComp && (
                  <span style={{ fontSize: '11px', color: '#4b5563' }}>{ownerComp.tool_label}</span>
                )}
              </div>
              <span style={{ fontSize: '11px', color: '#6b7280' }}>
                {ownerFiles} 个文件 · {formatTimestamp(ownerMtime)}
              </span>
            </div>
            <div style={{ fontSize: '11px', fontFamily: 'monospace', color: '#4b5563', marginTop: '5px', wordBreak: 'break-all' }}>
              {skill.source_ref || skill.central_path}
            </div>
            <div style={{ fontSize: '11px', color: '#9ca3af', marginTop: '4px' }}>
              下面各份副本没有主次之分：谁改过内容，谁就是最新，会被自动同步到其它副本。
            </div>
          </div>

          <div style={{ borderLeft: '1px solid #e5e7eb', marginLeft: '5px', paddingLeft: '18px', display: 'flex', flexDirection: 'column', gap: '10px' }}>

            {/* 链接组 */}
            <div style={{ position: 'relative', padding: '12px 14px', border: '1px solid #e5e7eb', borderRadius: '8px' }}>
              <span style={{ position: 'absolute', left: '-22px', top: '17px', width: '7px', height: '7px', borderRadius: '50%', background: '#fff', border: '1.5px solid #10b981' }} />
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px' }}>
                <span style={{ fontSize: '13px', fontWeight: 600 }}>
                  链接 <span style={{ fontWeight: 400, color: '#6b7280' }}>{linkComps.length} 个</span>
                </span>
                <span style={{ fontSize: '11px', color: '#0f6e56' }}>实时跟随，不需要同步</span>
              </div>
              <div style={{ fontSize: '11px', color: '#6b7280', marginTop: '6px', lineHeight: 1.7 }}>
                {linkSummary.length === 0
                  ? '暂无'
                  : linkSummary.map((item) => (
                      <span key={item.label}>
                        {item.label}
                        {item.count > 1 ? ` ×${item.count}` : ''}
                        {'  '}
                      </span>
                    ))}
              </div>
            </div>

            {/* 副本组：待处理的排在最前 */}
            <div style={{ position: 'relative', padding: '12px 14px', border: '1px solid #e5e7eb', borderRadius: '8px' }}>
              <span style={{ position: 'absolute', left: '-22px', top: '17px', width: '7px', height: '7px', borderRadius: '50%', background: '#fff', border: '1.5px solid #ef9f27' }} />
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px' }}>
                <span style={{ fontSize: '13px', fontWeight: 600 }}>
                  副本 <span style={{ fontWeight: 400, color: '#6b7280' }}>{copyComps.length} 个</span>
                </span>
                <span style={{ fontSize: '11px', color: '#6b7280' }}>需要同步，启动时自动比对</span>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '10px' }}>
                {copyRows.length === 0 && (
                  <div style={{ fontSize: '11px', color: '#9ca3af' }}>没有实体副本，全部是链接</div>
                )}
                {copyRows.map((comp) => {
                  const key = `${comp.tool}-${comp.instance}`
                  const isOperating = operatingTool === comp.tool
                  const tone =
                    comp.status === 'tool_newer'
                      ? { bg: 'rgba(245,158,11,.10)', border: '1px solid rgba(245,158,11,.45)' }
                      : comp.status === 'central_newer'
                        ? { bg: 'rgba(37,99,235,.06)', border: '1px solid rgba(37,99,235,.30)' }
                        : { bg: '#f9fafb', border: '1px solid #e5e7eb' }
                  return (
                    <div key={key} style={{ padding: '9px 11px', borderRadius: '6px', background: tone.bg, border: tone.border }}>
                      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px', flexWrap: 'wrap' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0, flexWrap: 'wrap' }}>
                          <span style={{ fontSize: '12px', fontWeight: 600 }}>
                            {comp.tool_label}
                            {comp.instance ? ` · ${comp.instance}` : ''}
                          </span>
                          <span
                            style={{
                              fontSize: '11px',
                              padding: '2px 7px',
                              borderRadius: '999px',
                              backgroundColor:
                                comp.status === 'synced'
                                  ? 'rgba(16,185,129,.14)'
                                  : comp.status === 'tool_newer'
                                    ? 'rgba(245,158,11,.22)'
                                    : 'rgba(37,99,235,.12)',
                              color:
                                comp.status === 'synced'
                                  ? '#059669'
                                  : comp.status === 'tool_newer'
                                    ? '#b45309'
                                    : '#2563eb',
                            }}
                          >
                            {comp.status === 'synced' ? '完全一致' : comp.diff_description}
                          </span>
                          <span style={{ fontSize: '11px', color: '#9ca3af' }}>
                            {comp.tool_file_count} 文件
                          </span>
                        </div>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                          {comp.status === 'central_newer' && (
                            <button
                              type="button"
                              className="btn btn-secondary"
                              disabled={isOperating}
                              onClick={() => void handleSyncDown(comp.tool, comp.tool_label)}
                              title="原件更新了，用原件的内容覆盖这一个工具"
                              style={{ fontSize: '11px', padding: '3px 9px', height: 'auto', borderRadius: '6px' }}
                            >
                              用原件覆盖此工具
                            </button>
                          )}
                          {comp.status === 'tool_newer' && (
                            <button
                              type="button"
                              className="btn btn-primary"
                              disabled={isOperating}
                              onClick={() => void handleAdoptAsLatest(comp)}
                              title="这一份更新，以它为准推给其它所有工具"
                              style={{ fontSize: '11px', padding: '3px 9px', height: 'auto', borderRadius: '6px' }}
                            >
                              用这份作为最新
                            </button>
                          )}
                          {comp.status === 'missing' && (
                            <button
                              type="button"
                              className="btn btn-secondary"
                              disabled={isOperating}
                              onClick={() => void handleSyncDown(comp.tool, comp.tool_label)}
                              style={{ fontSize: '11px', padding: '3px 9px', height: 'auto', borderRadius: '6px' }}
                            >
                              安装到此工具
                            </button>
                          )}
                          <button
                            type="button"
                            className="btn btn-secondary"
                            disabled={isOperating}
                            onClick={() => void handlePromoteToCentral(comp.tool, comp.tool_label)}
                            style={{ fontSize: '11px', padding: '3px 9px', height: 'auto', borderRadius: '6px' }}
                            title="把这个副本的内容更新到母库镜像（只是备份，不改原件）"
                          >
                            更新母库备份
                          </button>
                        </div>
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>

            {brokenComps.length > 0 && (
              <div style={{ position: 'relative', padding: '12px 14px', border: '1px solid rgba(226,75,74,.45)', borderRadius: '8px', background: 'rgba(226,75,74,.05)' }}>
                <span style={{ position: 'absolute', left: '-22px', top: '17px', width: '7px', height: '7px', borderRadius: '50%', background: '#fff', border: '1.5px solid #e24b4a' }} />
                <div style={{ fontSize: '13px', fontWeight: 600, color: '#a32d2d' }}>
                  需要处理 {brokenComps.length} 个
                </div>
                <div style={{ fontSize: '11px', color: '#b91c1c', marginTop: '5px', lineHeight: 1.7 }}>
                  {brokenComps.map((c) => `${c.tool_label}（${c.diff_description}）`).join(' · ')}
                </div>
              </div>
            )}

          </div>

          <div style={{ fontSize: '11px', color: '#9ca3af', lineHeight: 1.6 }}>
            链接不会落后——它们指向原件本身。只有副本需要比对，落后时旧内容会先移入 backup\stale-copies。
          </div>
        </div>

        <div
          className="modal-footer"
          style={{
            padding: '12px 20px',
            borderTop: '1px solid var(--color-border, #e5e7eb)',
            display: 'flex',
            justifyContent: 'flex-end',
            gap: '10px',
          }}
        >
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>
  )
})

SyncCompareModal.displayName = 'SyncCompareModal'
