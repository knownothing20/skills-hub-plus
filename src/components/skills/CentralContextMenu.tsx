import { AlertTriangle, FolderOpen, RefreshCw } from 'lucide-react'
import FloatingMenu, { MenuItem } from './FloatingMenu'
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
  const { skill, x, y } = state

  return (
    <FloatingMenu x={x} y={y} name="central" estimatedHeight={220} onClose={onClose}>
      <div className="tool-context-header">
        <div className="tool-context-header-info">
          <div className="tool-context-title">母库镜像</div>
          <div className={`tool-context-badge ${mirrorStale ? 'copy' : 'owner'}`}>
            {mirrorStale ? '落后于原件' : '与原件一致'}
          </div>
        </div>
      </div>

      <div className="tool-context-divider" />

      <MenuItem
        icon={
          mirrorStale ? (
            <AlertTriangle size={14} className="icon-copy" />
          ) : (
            <RefreshCw size={14} className="icon-copy" />
          )
        }
        title="用原件覆盖母库"
        hint={
          hasOwner
            ? '整目录替换为原件内容，local/ 等私有内容会被保留'
            : '没有登记原件目录，无法覆盖'
        }
        disabled={!hasOwner}
        onClick={() => {
          onClose()
          onRefreshMirror(skill.id)
        }}
      />

      <div className="tool-context-divider" />

      <MenuItem
        icon={<FolderOpen size={14} />}
        title="打开母库文件夹"
        hint={
          <span className="path-preview" title={skill.central_path}>
            {skill.central_path}
          </span>
        }
        onClick={() => {
          onClose()
          if (skill.central_path) onOpenFolder(skill.central_path)
        }}
      />
    </FloatingMenu>
  )
}