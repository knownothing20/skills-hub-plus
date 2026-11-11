import { memo } from 'react'
import TitleBar from './TitleBar'
import Sidebar from './Sidebar'
import type { TFunction } from 'i18next'

type ManagementTab = 'tags' | 'tools' | 'updates'

type HeaderProps = {
  activeView: 'myskills' | 'explore' | 'detail' | 'settings' | 'manage' | 'library'
  managementTab: ManagementTab
  skillCount: number
  tagCount: number
  toolCount: number
  updateCount: number
  collapsed: boolean
  onToggleCollapsed: () => void
  onOpenSettings: () => void
  onViewChange: (view: 'myskills' | 'explore' | 'manage' | 'library') => void
  onManagementTabChange: (tab: ManagementTab) => void
  t: TFunction
}

const Header = ({
  activeView,
  managementTab,
  skillCount,
  tagCount,
  toolCount,
  updateCount,
  collapsed,
  onToggleCollapsed,
  onOpenSettings,
  onViewChange,
  onManagementTabChange,
  t,
}: HeaderProps) => (
  <>
    <TitleBar t={t} />
    <Sidebar
      activeView={activeView}
      managementTab={managementTab}
      skillCount={skillCount}
      tagCount={tagCount}
      toolCount={toolCount}
      updateCount={updateCount}
      collapsed={collapsed}
      onToggleCollapsed={onToggleCollapsed}
      onOpenSettings={onOpenSettings}
      onViewChange={onViewChange}
      onManagementTabChange={onManagementTabChange}
      t={t}
    />
  </>
)

export { TitleBar, Sidebar }
export default memo(Header)
