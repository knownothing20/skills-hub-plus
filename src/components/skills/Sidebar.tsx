import { memo, type PointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  ChevronLeft,
  Compass,
  Layers3,
  Library,
  RefreshCw,
  Settings,
  Tag,
  Wrench,
} from 'lucide-react'
import type { TFunction } from 'i18next'

type ManagementTab = 'tags' | 'tools' | 'updates'

type SidebarProps = {
  activeView: 'myskills' | 'explore' | 'detail' | 'settings' | 'manage' | 'library'
  managementTab: ManagementTab
  skillCount: number
  tagCount: number
  toolCount: number
  collapsed: boolean
  onToggleCollapsed: () => void
  onOpenSettings: () => void
  onViewChange: (view: 'myskills' | 'explore' | 'manage' | 'library') => void
  onManagementTabChange: (tab: ManagementTab) => void
  t: TFunction
}

const startWindowDrag = (event: PointerEvent<HTMLElement>) => {
  if (event.button !== 0 || !event.isPrimary) return
  const target = event.target as HTMLElement
  if (target.closest('button, input, select, textarea, a, [role="button"]')) return
  event.preventDefault()
  void getCurrentWindow().startDragging().catch(() => undefined)
}

const Sidebar = ({
  activeView,
  managementTab,
  skillCount,
  tagCount,
  toolCount,
  collapsed,
  onToggleCollapsed,
  onOpenSettings,
  onViewChange,
  onManagementTabChange,
  t,
}: SidebarProps) => (
  <aside className={`skills-sidebar${collapsed ? ' collapsed' : ''}`}>
    <div
      className="sidebar-brand"
      data-tauri-drag-region
      onPointerDown={startWindowDrag}
    >
      <div className="sidebar-logo" data-tauri-drag-region aria-hidden="true">
        <span className="sidebar-logo-mark" data-tauri-drag-region />
      </div>
      <div className="sidebar-brand-copy" data-tauri-drag-region>
        <strong data-tauri-drag-region>{t('appName')}</strong>
        <span data-tauri-drag-region>{t('workspaceSubtitle')}</span>
      </div>
      <button
        className="sidebar-collapse"
        type="button"
        onClick={onToggleCollapsed}
        aria-label={collapsed ? t('sidebar.expand') : t('sidebar.collapse')}
        title={collapsed ? t('sidebar.expand') : t('sidebar.collapse')}
      >
        <ChevronLeft size={collapsed ? 14 : 16} />
      </button>
    </div>

    <div className="sidebar-section-label">{t('workspace')}</div>
    <nav className="sidebar-nav" aria-label={t('workspace')}>
      <button
        className={activeView === 'myskills' || activeView === 'detail' ? 'active' : ''}
        type="button"
        onClick={() => onViewChange('myskills')}
        title={collapsed ? t('navMySkills') : undefined}
      >
        <Layers3 size={18} />
        <span>{t('navMySkills')}</span>
        {!collapsed && <em>{skillCount}</em>}
      </button>
      <button
        className={activeView === 'library' ? 'active' : ''}
        type="button"
        onClick={() => onViewChange('library')}
        title={collapsed ? t('centralLibrary', { defaultValue: '母库' }) : undefined}
      >
        <Library size={18} />
        <span>{t('centralLibrary', { defaultValue: '母库' })}</span>
      </button>
      <button
        className={activeView === 'explore' ? 'active' : ''}
        type="button"
        onClick={() => onViewChange('explore')}
        title={collapsed ? t('addSkills') : undefined}
      >
        <Compass size={18} />
        <span>{t('addSkills')}</span>
      </button>
    </nav>

    <div className="sidebar-section-label">{t('navManageCenter')}</div>
    <nav className="sidebar-nav" aria-label={t('navManageCenter')}>
      <button
        className={activeView === 'manage' && managementTab === 'tags' ? 'active' : ''}
        type="button"
        onClick={() => onManagementTabChange('tags')}
        title={collapsed ? t('manageTabs.tags') : undefined}
      >
        <Tag size={18} />
        <span>{t('manageTabs.tags')}</span>
        {!collapsed && <em>{tagCount}</em>}
      </button>
      <button
        className={activeView === 'manage' && managementTab === 'tools' ? 'active' : ''}
        type="button"
        onClick={() => onManagementTabChange('tools')}
        title={collapsed ? t('manageTabs.tools') : undefined}
      >
        <Wrench size={18} />
        <span>{t('manageTabs.tools')}</span>
        {!collapsed && <em>{toolCount}</em>}
      </button>
      <button
        className={activeView === 'manage' && managementTab === 'updates' ? 'active' : ''}
        type="button"
        onClick={() => onManagementTabChange('updates')}
        title={collapsed ? t('manageTabs.updates') : undefined}
      >
        <RefreshCw size={18} />
        <span>{t('manageTabs.updates')}</span>

      </button>
    </nav>

    <div className="sidebar-spacer" />
    <button
      className={`sidebar-settings${activeView === 'settings' ? ' active' : ''}`}
      type="button"
      onClick={onOpenSettings}
      title={collapsed ? t('settings') : undefined}
    >
      <Settings size={18} />
      <span>{t('settings')}</span>
    </button>
  </aside>
)

export default memo(Sidebar)
