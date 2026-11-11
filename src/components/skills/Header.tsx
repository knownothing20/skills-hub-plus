import { memo } from 'react'
import TitleBar from './TitleBar'
import Sidebar from './Sidebar'
import type { TFunction } from 'i18next'

type ManagementTab = 'tags' | 'tools'

type HeaderProps = {
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

const Header = ({
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
}: HeaderProps) => (
  <>
    <TitleBar t={t} />
    <Sidebar
      activeView={activeView}
      managementTab={managementTab}
      skillCount={skillCount}
      tagCount={tagCount}
      toolCount={toolCount}
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
