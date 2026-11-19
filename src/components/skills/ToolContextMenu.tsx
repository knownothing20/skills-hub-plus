import { Box, Crown, Trash2, FolderOpen } from 'lucide-react'
import FloatingMenu, { MenuItem } from './FloatingMenu'
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
  const { tool, skill, synced, isOwner, instance, targetPath, x, y } = state

  return (
    <FloatingMenu x={x} y={y} name="tool" onClose={onClose}>
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

      {/* 未同步状态：提供选择以实体副本同步 */}
      {!synced && (
        <MenuItem
          icon={<Box size={14} className="icon-copy" />}
          title="同步为 实体副本 (Copy)"
          hint="独立文件目录，配置互不干扰"
          onClick={() => {
            onClose()
            onSync(skill, tool.id)
          }}
        />
      )}

      {/* 已同步为实体副本：提供取消同步 */}
      {synced && !isOwner && (
        <MenuItem
          icon={<Trash2 size={14} />}
          title="取消同步 (移除该软件技能)"
          danger
          onClick={() => {
            onClose()
            onUnsync(skill, tool.id)
          }}
        />
      )}

      {/* 主 skill 归属：按当前形态给不同说明 */}
      {synced && onPromoteToOwner && (
        <>
          <div className="tool-context-divider" />
          {isOwner ? (
            <MenuItem
              icon={<Crown size={14} />}
              title="当前为主 skill"
              hint="原件就在这个目录，其它软件都以它为准"
              disabled
            />
          ) : (
            <MenuItem
              icon={<Crown size={14} />}
              title="设为主 skill"
              hint="直接以这个实体副本为原件，文件不动"
              onClick={() => {
                onClose()
                onPromoteToOwner(skill, tool.id, instance)
              }}
            />
          )}
        </>
      )}

      {/* 打开目标目录 */}
      {targetPath && onOpenFolder && (
        <>
          <div className="tool-context-divider" />
          <MenuItem
            icon={<FolderOpen size={14} />}
            title="在资源管理器中打开"
            hint={
              <span className="path-preview" title={targetPath}>
                {targetPath}
              </span>
            }
            onClick={() => {
              onClose()
              onOpenFolder(targetPath)
            }}
          />
        </>
      )}
    </FloatingMenu>
  )
}