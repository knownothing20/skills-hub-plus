import { useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { AlertTriangle, FolderOpen, RefreshCw } from 'lucide-react'
import type { ManagedSkill } from './types'

/**
 * 母库角标的右键菜单。
 *
 * 存在的理由：母库镜像刷新（owner → 母库）没有自动同步，是唯一必须手动触发的
 * 方向。原先只有一个 `!`/版本字母角标，而字母由 version_labels 决定，指纹收敛后
 * 就消失，入口随之不见。这里把入口挂到恒存在的母库图标上，不依赖任何状态判定。
 */
export type CentralContextMenuState = {
  x: number
  y: number
  skill: ManagedSkill
}

type CentralContextMenuProps = {
  state: CentralContextMenuState
  onClose: () => void
  onRefreshMirror: (skillId: string) => void
  onOpenFolder: (path: string) => void
  /** 母库是否落后于原件；仅用于文案与高亮，不影响入口可用性 */
  mirrorStale?: boolean
  /** 是否具备「原件」可比对象（source_type=local 且有 source_ref） */
  hasOwner: boolean
}

export default function CentralContextMenu({
  state,
  onClose,
  onRefreshMirror,
  onOpenFolder,
  mirrorStale,
  hasOwner,
}: CentralContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)
  const { skill, x, y } = state

  useEffect(() => {
    const handlePointerDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    const handleScroll = (e: Event) => {
      if (menuRef.current && menuRef.current.contains(e.target as Node)) return
      onClose()
    }

    // 延迟 50ms 挂载外部点击关闭，避免第一下右键操作本身的后续 pointer 事件意外关闭
    const timer = window.setTimeout(() => {
      window.addEventListener('pointerdown', handlePointerDown)
    }, 50)
    window.addEventListener('keydown', handleKeyDown)
    window.addEventListener('scroll', handleScroll, true)
    window.addEventListener('resize', onClose)

    return () => {
      window.clearTimeout(timer)
      window.removeEventListener('pointerdown', handlePointerDown)
      window.removeEventListener('keydown', handleKeyDown)
      window.removeEventListener('scroll', handleScroll, true)
      window.removeEventListener('resize', onClose)
    }
  }, [onClose])

  let left = x
  let top = y
  if (typeof window !== 'undefined') {
    const menuWidth = 260
    const menuHeight = 220
    if (left + menuWidth > window.innerWidth - 12) {
      left = Math.max(12, window.innerWidth - menuWidth - 12)
    }
    if (top + menuHeight > window.innerHeight - 12) {
      top = Math.max(12, window.innerHeight - menuHeight - 12)
    }
  }

  const content = (
    <div
      ref={menuRef}
      className="tool-context-menu"
      style={{ left, top }}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
      }}
    >
      <div className="tool-context-header">
        <div className="tool-context-header-info">
          <div className="tool-context-title">母库镜像</div>
          <div className={`tool-context-badge ${mirrorStale ? 'copy' : 'owner'}`}>
            {mirrorStale ? '落后于原件' : '与原件一致'}
          </div>
        </div>
      </div>

      <div className="tool-context-divider" />

      <button
        type="button"
        className="tool-context-item"
        disabled={!hasOwner}
        onClick={() => {
          onClose()
          onRefreshMirror(skill.id)
        }}
      >
        {mirrorStale ? (
          <AlertTriangle size={14} className="icon-copy" />
        ) : (
          <RefreshCw size={14} className="icon-copy" />
        )}
        <div className="item-text">
          <span>用原件覆盖母库</span>
          <small>
            {hasOwner
              ? '整目录替换为原件内容，local/ 等私有内容会被保留'
              : '没有登记原件目录，无法覆盖'}
          </small>
        </div>
      </button>

      <div className="tool-context-divider" />

      <button
        type="button"
        className="tool-context-item"
        onClick={() => {
          onClose()
          if (skill.central_path) onOpenFolder(skill.central_path)
        }}
      >
        <FolderOpen size={14} />
        <div className="item-text">
          <span>打开母库文件夹</span>
          <small className="path-preview" title={skill.central_path}>
            {skill.central_path}
          </small>
        </div>
      </button>
    </div>
  )

  if (typeof document === 'undefined') return null
  return createPortal(content, document.body)
}