import { useEffect, useRef, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

/**
 * 右键菜单的共用浮层：定位夹取、外部点击/Esc/滚动/尺寸变化关闭、
 * 以及键盘导航（↑↓ 移动、Enter 激活、Esc 关闭）。
 *
 * 抽出来的理由：母库菜单和工具菜单原先各写了一遍同样的 60 行，且两份都
 * 没有键盘支持——右键是这两个菜单的唯一入口，没有键盘就意味着只能用鼠标。
 */
type FloatingMenuProps = {
  x: number
  y: number
  /** 供 Automation/测试定位，落在 data-menu 上 */
  name: string
  /** 菜单大致高度，用于贴底时上翻；实际尺寸偏小不影响，夹取有 12px 余量 */
  estimatedHeight?: number
  onClose: () => void
  children: ReactNode
}

export default function FloatingMenu({
  x,
  y,
  name,
  estimatedHeight = 240,
  onClose,
  children,
}: FloatingMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handlePointerDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
        return
      }
      // 方向键在可选项之间移动焦点；跳过 disabled 项。
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        const items = focusableItems(menuRef.current)
        if (items.length === 0) return
        e.preventDefault()
        const current = items.indexOf(document.activeElement as HTMLElement)
        const step = e.key === 'ArrowDown' ? 1 : -1
        const next = current === -1 ? 0 : (current + step + items.length) % items.length
        items[next].focus()
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

  // 打开时把焦点送进菜单，键盘用户不必先点一下
  useEffect(() => {
    const items = focusableItems(menuRef.current)
    items[0]?.focus()
  }, [])

  let left = x
  let top = y
  if (typeof window !== 'undefined') {
    const menuWidth = 260
    if (left + menuWidth > window.innerWidth - 12) {
      left = Math.max(12, window.innerWidth - menuWidth - 12)
    }
    if (top + estimatedHeight > window.innerHeight - 12) {
      top = Math.max(12, window.innerHeight - estimatedHeight - 12)
    }
  }

  if (typeof document === 'undefined') return null

  return createPortal(
    <div
      ref={menuRef}
      className="tool-context-menu"
      style={{ left, top }}
      role="menu"
      aria-label={name}
      data-menu={name}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
      }}
    >
      {children}
    </div>,
    document.body,
  )
}

function focusableItems(root: HTMLElement | null): HTMLElement[] {
  if (!root) return []
  return Array.from(
    root.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])'),
  )
}

/** 菜单项：统一 role/tabIndex，让方向键与 Enter 都能用 */
export function MenuItem({
  icon,
  title,
  hint,
  danger,
  disabled,
  onClick,
}: {
  icon: ReactNode
  title: ReactNode
  hint?: ReactNode
  danger?: boolean
  disabled?: boolean
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      role="menuitem"
      tabIndex={disabled ? -1 : 0}
      className={`tool-context-item${danger ? ' danger' : ''}`}
      disabled={disabled}
      onClick={onClick}
    >
      {icon}
      <div className="item-text">
        <span>{title}</span>
        {hint ? <small>{hint}</small> : null}
      </div>
    </button>
  )
}