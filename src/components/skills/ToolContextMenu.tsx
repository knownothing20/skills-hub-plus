import { useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { Box, Crown, Trash2, FolderOpen } from 'lucide-react'
import ToolIcon from './ToolIcon'
import type { ManagedSkill, ToolOption } from './types'

export type ToolContextMenuState = {
  x: number
  y: number
  tool: ToolOption
  skill: ManagedSkill
  synced: boolean
  isOwner?: boolean
  /** 后端判定的角色：owner / copy / link_to_owner / link_to_central / link_other / missing */
  role?: string
  /** 多账号工具的账号标识；主实例为空 */
  instance?: string
  targetPath?: string
}

type ToolContextMenuProps = {
  state: ToolContextMenuState
  onClose: () => void
  onSync: (skill: ManagedSkill, toolId: string) => void
  onUnsync: (skill: ManagedSkill, toolId: string) => void
  onPromoteToOwner?: (skill: ManagedSkill, toolId: string, instance?: string) => void
  onOpenFolder?: (path: string) => void
}

export default function ToolContextMenu({
  state,
  onClose,
  onSync,
  onUnsync,
  onPromoteToOwner,
  onOpenFolder,
}: ToolContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)
  const { tool, skill, synced, isOwner, instance, targetPath, x, y } = state

  useEffect(() => {
    const handlePointerDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      }
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

  // Calculate clamped coordinates
  let left = x
  let top = y
  if (typeof window !== 'undefined') {
    const menuWidth = 240
    const menuHeight = 240
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
        <ToolIcon toolKey={tool.id} label={tool.label} avatar={tool.avatar} />
        <div className="tool-context-header-info">
          <div className="tool-context-title">{tool.label}</div>
          <div
            className={`tool-context-badge ${synced ? (isOwner ? 'owner' : 'copy') : 'none'}`}
          >
            {synced ? (isOwner ? '原件（本地源）' : '已同步 · 实体副本') : '未同步'}
          </div>
        </div>
      </div>

      <div className="tool-context-divider" />

      {/* 未同步状态：提供选择以软链接或实体副本同步 */}
      {!synced && (
        <>
          <button
            type="button"
            className="tool-context-item"
            onClick={() => {
              onClose()
              onSync(skill, tool.id)
            }}
          >
            <Box size={14} className="icon-copy" />
            <div className="item-text">
              <span>同步为 实体副本 (Copy)</span>
              <small>独立文件目录，配置互不干扰</small>
            </div>
          </button>
        </>
      )}

      {/* 已同步为实体副本：提供取消同步 */}
      {synced && !isOwner && (
        <>
          <button
            type="button"
            className="tool-context-item danger"
            onClick={() => {
              onClose()
              onUnsync(skill, tool.id)
            }}
          >
            <Trash2 size={14} />
            <div className="item-text">
              <span>取消同步 (移除该软件技能)</span>
            </div>
          </button>
        </>
      )}

      {/* 主 skill 归属：按当前形态给不同说明 */}
      {synced && onPromoteToOwner && (
        <>
          <div className="tool-context-divider" />
          {isOwner ? (
            <button type="button" className="tool-context-item" disabled>
              <Crown size={14} />
              <div className="item-text">
                <span>当前为主 skill</span>
                <small>原件就在这个目录，其它软件都以它为准</small>
              </div>
            </button>
          ) : (
            <button
              type="button"
              className="tool-context-item"
              onClick={() => {
                onClose()
                onPromoteToOwner(skill, tool.id, instance)
              }}
            >
              <Crown size={14} />
              <div className="item-text">
                <span>设为主 skill</span>
                <small>
                  直接以这个实体副本为原件，文件不动
                </small>
              </div>
            </button>
          )}
        </>
      )}

      {/* 打开目标目录 */}
      {targetPath && onOpenFolder && (
        <>
          <div className="tool-context-divider" />
          <button
            type="button"
            className="tool-context-item"
            onClick={() => {
              onClose()
              onOpenFolder(targetPath)
            }}
          >
            <FolderOpen size={14} />
            <div className="item-text">
              <span>在资源管理器中打开</span>
              <small className="path-preview" title={targetPath}>
                {targetPath}
              </small>
            </div>
          </button>
        </>
      )}
    </div>
  )

  if (typeof document === 'undefined') return null
  return createPortal(content, document.body)
}

