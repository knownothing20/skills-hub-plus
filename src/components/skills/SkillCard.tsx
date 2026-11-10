import { memo, useState } from 'react'
import { Copy, RefreshCw, Tag, Trash2, Link2, Box, ShieldOff, Crown, Share2 } from 'lucide-react'
import { openPath } from '@tauri-apps/plugin-opener'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import { getToolSyncState, isActiveSkillTarget } from './skillSyncStatus'
import { roleOfTarget } from './skillOwner'
import { isSkillUpdateable } from './skillUpdateability'
import type {
  ManagedSkill,
  SharedSkillDirDto,
  SkillHealthDto,
  StaleCopyStatusDto,
  ToolOption,
} from './types'
import logoMark from '../../assets/logo-mark.svg'
import SkillIcon from './SkillIcon'
import ToolIcon from './ToolIcon'
import ToolContextMenu, { type ToolContextMenuState } from './ToolContextMenu'

type GithubInfo = { label: string; href: string }

type SkillCardProps = {
  skill: ManagedSkill
  installedTools: ToolOption[]
  loading: boolean
  bulkMode: boolean
  bulkSelected: boolean
  health?: SkillHealthDto
  getGithubInfo: (url: string | null | undefined) => GithubInfo | null
  getSkillSourceLabel: (skill: ManagedSkill) => string
  formatRelative: (ms: number | null | undefined) => string
  onUpdate: (skill: ManagedSkill) => void
  onDelete: (skillId: string) => void
  onIgnore?: (skillId: string) => void
  onToggleEnabled: (skill: ManagedSkill) => void
  onToggleTool: (skill: ManagedSkill, toolId: string) => void
  onSyncToolWithMode?: (skill: ManagedSkill, toolId: string, mode: 'copy' | 'junction') => void
  onUnsyncTool?: (skill: ManagedSkill, toolId: string) => void
  onPromoteToolToOwner?: (skill: ManagedSkill, toolId: string, instance?: string) => void
  onOpenScope: (skill: ManagedSkill) => void
  onOpenDetail: (skill: ManagedSkill) => void
  onEditTags: (skill: ManagedSkill) => void
  onToggleBulkSelection: (skillId: string) => void
  getSkillScope: (skill: ManagedSkill) => 'global' | 'project'
  getSkillProjects: (skill: ManagedSkill) => string[]
  staleBySkillTool?: Map<string, StaleCopyStatusDto>
  onRefreshStale?: (skillId: string, toolId: string) => Promise<void>
  sharedDirs?: SharedSkillDirDto[]
  /** 每个工具实际有几个账号目录（仅统计存在的） */
  toolInstanceTotals?: Record<string, number>
  toolCounts?: Record<string, number>
  t: TFunction
}

const SkillCard = ({
  skill,
  installedTools,
  toolCounts,
  loading,
  bulkMode,
  bulkSelected,
  health,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onIgnore,
  onToggleEnabled,
  onToggleTool,
  onSyncToolWithMode,
  onUnsyncTool,
  onPromoteToolToOwner,
  onOpenScope,
  onOpenDetail,
  onEditTags,
  onToggleBulkSelection,
  getSkillScope,
  getSkillProjects,
  staleBySkillTool,
  onRefreshStale,
  sharedDirs,
  toolInstanceTotals,
  t,
}: SkillCardProps) => {
  const [contextMenu, setContextMenu] = useState<ToolContextMenuState | null>(null)

  const github = getGithubInfo(skill.source_ref)
  const sourceLabel = github?.label ?? getSkillSourceLabel(skill)
  const copyValue = (github?.href ?? skill.source_ref ?? skill.central_path).trim()
  const description = skill.description?.trim() || ''
  const scope = getSkillScope(skill)
  const projectCount = getSkillProjects(skill).length
  const enabled = skill.enabled !== false
  const updateable = isSkillUpdateable(skill)
  const scopeTargets = skill.targets.filter(
    (target) => (target.scope ?? 'global') === scope,
  )
  const ownerToolId = scopeTargets.find(
    (target) => roleOfTarget(target) === 'owner',
  )?.tool

  // 共享目录曝光：技能登记了 agents 目标 → 内容同时对所有读取者生效
  const normalizePath = (value: string) =>
    value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
  const sharedTarget = scopeTargets.find(
    (target) => target.tool === 'agents' && isActiveSkillTarget(target),
  )
  const sharedDir = sharedTarget
    ? sharedDirs?.find((dir) =>
        normalizePath(sharedTarget.target_path).startsWith(normalizePath(dir.path)),
      )
    : undefined
  const sharedReadersLabel = sharedDir
    ? sharedDir.readers
        .map((key) => t(`tools.${key}`, { defaultValue: key }))
        .join('、')
    : ''
  const hasOwner =
    skill.source_type === 'local' && Boolean(skill.source_ref?.trim())
  const centralBadgeLabel = hasOwner
    ? t('centralBadgeMirror')
    : t('centralBadgeLocal')
  const centralBadgeTitle = hasOwner
    ? t('centralBadgeMirrorHint', {
        origin: skill.source_ref?.trim() ?? '',
        path: skill.central_path ?? '',
      })
    : t('centralBadgeLocalHint', { path: skill.central_path ?? '' })
  const handleCopySource = async () => {
    if (!copyValue) return
    try {
      await navigator.clipboard.writeText(copyValue)
      toast.success(t('copied'))
    } catch {
      toast.error(t('copyFailed'))
    }
  }

  return (
    <div
      className={`skill-card${bulkMode ? ' bulk-mode' : ''}${bulkSelected ? ' bulk-selected' : ''}${enabled ? '' : ' disabled-skill'}`}
      onClick={(event) => {
        if (bulkMode) return
        // 整行可点进详情，但行内的按钮/输入（图标、开关、标签、删除…）保持自己的行为
        const target = event.target as HTMLElement
        if (target.closest('button, a, input, textarea, select, label, [role="button"]')) return
        onOpenDetail(skill)
      }}
    >
      {bulkMode ? (
        <label className="bulk-skill-check" aria-label={t('bulk.toggleSkill')}>
          <input
            type="checkbox"
            checked={bulkSelected}
            onChange={() => onToggleBulkSelection(skill.id)}
            disabled={loading}
          />
          <span />
        </label>
      ) : null}

      <div className="skill-card-head">
        <div className="skill-identity">
          <SkillIcon
            name={skill.name}
            iconDataUrl={skill.icon_data_url}
            brandColor={skill.brand_color}
          />
          <div className="skill-identity-copy">
            <div className="skill-title-line">
              <button className="skill-name clickable" type="button" onClick={() => onOpenDetail(skill)}>
                {skill.name}
              </button>
              <button
                className="skill-source-copy"
                type="button"
                title={`${t('copy')}：${sourceLabel}`}
                aria-label={`${t('copy')}：${sourceLabel}`}
                onClick={() => void handleCopySource()}
                disabled={!copyValue}
              >
                <Copy size={12} />
              </button>
            </div>
            {description ? (
              <div className="skill-description" title={description}>
                {description}
              </div>
            ) : null}
          </div>
        </div>

        <button
          className={`skill-switch${enabled ? ' enabled' : ''}`}
          type="button"
          onClick={() => onToggleEnabled(skill)}
          disabled={loading}
          aria-label={enabled ? t('disableSkill') : t('enableSkill')}
        ><span /></button>
      </div>

      <div className="skill-card-meta">
        <div className="skill-tags-inline">
          {skill.tags.length > 0 ? skill.tags.slice(0, 2).map((tag) => (
            <button key={tag.id} className="skill-tag-pill" type="button" onClick={() => onEditTags(skill)}>
              {tag.name}
            </button>
          )) : <span className="table-empty">{t('untagged')}</span>}
        </div>

        <button className={`scope-badge ${scope}`} type="button" onClick={() => onOpenScope(skill)}>
          {scope === 'project' ? t('scope.projectCount', { count: projectCount }) : t('scope.globalBadge')}
        </button>

        {health && health.broken.length > 0 ? (
          <span
            className="skill-health-badge is-broken"
            title={health.broken
              .map((broken) =>
                `${broken.tool}: ${broken.path} (${t(`health.reason.${broken.reason}`, { defaultValue: broken.reason })})`,
              )
              .join('\n')}
          >
            {t('health.cardBroken', { count: health.broken.length })}
          </span>
        ) : null}

        {health?.idle ? (
          <span className="skill-health-badge is-idle" title={t('health.idleHint')}>
            {t('health.cardIdle')}
          </span>
        ) : null}

        <button
          className="skill-central-badge"
          type="button"
          aria-label={centralBadgeLabel}
          title={centralBadgeTitle}
          onClick={(e) => {
            e.stopPropagation()
            if (skill.central_path) {
              openPath(skill.central_path).catch(() => {
                toast.error(t('openMasterFolderFailed'))
              })
            }
          }}
        >
          <img src={logoMark} alt={centralBadgeLabel} className="central-badge-logo" />
        </button>

        {sharedDir ? (
          <button
            className="skill-shared-badge"
            type="button"
            aria-label={t('shared.badgeLabel')}
            title={`${t('shared.badgeLabel')}\n${sharedDir.path}\n${t('shared.readersTitle', {
              readers: sharedReadersLabel,
            })}`}
            onClick={(e) => {
              e.stopPropagation()
              toast.info(t('shared.badgeLabel'), {
                description: `${sharedDir.path}\n${t('shared.readersTitle', {
                  readers: sharedReadersLabel,
                })}`,
                duration: 5000,
              })
            }}
          >
            <Share2 size={10} />
          </button>
        ) : null}

        <div className="skill-updated">{formatRelative(skill.updated_at)}</div>
      </div>

      <div className="skill-card-footer">
        <div className="skill-tools-block">
          {(() => {
            const externalTools = installedTools.filter((t) => {
              if (t.id === 'agents') return false
              const synced = getToolSyncState(skill, t.id, scope) === 'synced'
              if (synced) return true
              // 共享可达的工具也要出现在原位（虚线样式），不能挪到行尾。
              if (sharedDir && sharedDir.readers.includes(t.id)) return true
              // 如果软件在全库中一个 skill 都没有，默认隐藏该工具的占位图标
              return (toolCounts?.[t.id] ?? 0) > 0
            })
            const ownerToolLabel = ownerToolId
              ? (externalTools.find((tool) => tool.id === ownerToolId)?.label ??
                ownerToolId)
              : null

            return (
              <>
                <div className="skill-tool-avatars">
                  {externalTools.map((tool) => {
                    const syncState = getToolSyncState(skill, tool.id, scope)
                    const synced = syncState === 'synced'
                    const sharedOnly =
                      !synced && Boolean(sharedDir?.readers.includes(tool.id))
                    if (sharedOnly) {
                      return (
                        <button
                          key={`shared-${tool.id}`}
                          className="tool-sync-btn is-shared"
                          type="button"
                          title={t('shared.implicitTitle', { tool: tool.label })}
                          aria-label={t('shared.implicitTitle', { tool: tool.label })}
                          onClick={() => {
                            toast.info(
                              t('shared.implicitHint', { tool: tool.label }),
                              { duration: 3600 },
                            )
                          }}
                        >
                          <ToolIcon
                            toolKey={tool.id}
                            label={tool.label}
                            avatar={tool.avatar}
                          />
                        </button>
                      )
                    }

              const target = skill.targets.find(
                (t) =>
                  t.tool === tool.id &&
                  (t.scope ?? 'global') === scope &&
                  isActiveSkillTarget(t),
              )
              // 没有 target 表示该工具尚未安装这个技能，不应算作异常角色
              const role = target ? roleOfTarget(target) : null
              // 物理形态看 target 记录的 mode，不看 role：role 说的是「这个目录和原件/中心母版的关系」，
              // 而原件自己（role='owner'）在磁盘上同样有 mode——codex 的实体目录就是 mode='copy'。
              // 用 role 反推会把原件一律说成「软链接」，而它明明是实体。
              const isCopy = target?.mode === 'copy'
              const roleLabel =
                role === 'owner'
                  ? t('roleOwner')
                  : role === 'copy'
                    ? t('roleCopy')
                    : role === 'missing' || role === 'link_other'
                      ? t('roleBroken')
                      : role === 'link_to_owner' || role === 'link_to_central'
                        ? t('roleLink')
                        : ''
              const modeLabel = isCopy
                ? '实体副本 (Copy)'
                : target?.mode === 'junction'
                  ? '软链接 (Junction)'
                  : '软链接 (Symlink)'
              const staleInfo = target
                ? staleBySkillTool?.get(`${skill.name}/${tool.id}`)
                : undefined

              // 多账号工具：图标右上角直接标出「已覆盖账号数 / 账号总数」
              const instanceCount = scopeTargets.filter(
                (tg) => tg.tool === tool.id && isActiveSkillTarget(tg),
              ).length
              const instanceTotal = toolInstanceTotals?.[tool.id] ?? 1
              // 只在「这个技能确实进了至少一个账号目录」时才标数字。
              // 否则多账户工具会给整库每个技能都挂一个 0，噪音太大。
              const showInstanceBadge = instanceTotal > 1 && instanceCount > 0
              const instanceSuffix = showInstanceBadge
                ? ` · ${t('toolManagement.instanceCoverage', {
                    covered: instanceCount,
                    total: instanceTotal,
                  })}`
                : ''

              let finalClass: string = syncState
              let stateLabel = ''
              if (synced) {
                // 原件（主技能）不是「已同步的副本」：它本来就在这儿，绿色只表示就位。
                finalClass =
                  role === 'owner'
                    ? 'synced is-owner'
                    : isCopy
                      ? 'synced is-copy'
                      : 'synced is-symlink'
                stateLabel = role === 'owner' ? '原件（本地源）' : `已同步 · ${modeLabel}`
              } else if (syncState === 'failed') {
                finalClass = 'failed'
                stateLabel = t('toolManagement.syncFailed')
              } else if (syncState === 'partial') {
                finalClass = 'partial'
                stateLabel = t('toolManagement.syncPartialFailed')
              } else {
                finalClass = 'not-synced'
                stateLabel = '未安装 (点击同步)'
              }

              return (
                <button
                  key={tool.id}
                  className={`tool-sync-btn ${finalClass}${role ? ` role-${role}` : ''}${
                    staleInfo ? ' has-stale' : ''
                  }`}
                  type="button"
                  title={`${tool.label}${
                    role !== 'owner' && roleLabel ? ` · ${roleLabel}` : ''
                  } · ${
                    staleInfo
                      ? staleInfo.verdict === 'behind'
                        ? t('stale.behindTitle')
                        : staleInfo.verdict === 'hand_edited'
                          ? t('stale.handEditedTitle')
                          : t('stale.conflictTitle')
                      : stateLabel
                  }${instanceSuffix} (右键更多操作)`}
                  aria-label={`${tool.label}${
                    role !== 'owner' && roleLabel ? ` · ${roleLabel}` : ''
                  } · ${stateLabel}`}
                  aria-pressed={synced}
                  onClick={() => {
                    if (!enabled) return
                    if (staleInfo) {
                      if (staleInfo.verdict === 'behind') {
                        void onRefreshStale?.(skill.id, tool.id)
                      } else {
                        onOpenDetail(skill)
                      }
                      return
                    }
                    onToggleTool(skill, tool.id)
                  }}
                  onContextMenu={(e) => {
                    e.preventDefault()
                    e.stopPropagation()
                    if (!enabled) return
                    setContextMenu({
                      x: e.clientX,
                      y: e.clientY,
                      tool,
                      skill,
                      synced,
                      isCopy,
                      isOwner: role === 'owner',
                      role: role ?? undefined,
                      instance: target?.instance,
                      targetPath: target?.target_path,
                    })
                  }}
                  disabled={!enabled}
                >
                  <ToolIcon
                    toolKey={tool.id}
                    label={tool.label}
                    avatar={tool.avatar}
                  />
                  {staleInfo ? (
                    <span className="stale-badge" aria-hidden="true">
                      !
                    </span>
                  ) : null}
                  {showInstanceBadge ? (
                    <span
                      className={`instance-badge${
                        instanceCount >= instanceTotal ? '' : ' is-partial'
                      }`}
                      aria-hidden="true"
                    >
                      {instanceCount}
                    </span>
                  ) : null}
                  {role === 'owner' && (
                    <span
                      className="owner-crown"
                      title={
                        ownerToolLabel
                          ? t('roleOwnerAt', { tool: ownerToolLabel })
                          : t('roleOwner')
                      }
                    >
                      <Crown size={9} />
                    </span>
                  )}
                  {synced && role !== 'owner' && (
                    <span
                      className={`sync-mode-badge ${isCopy ? 'badge-copy' : 'badge-symlink'}`}
                      aria-hidden="true"
                    >
                      {isCopy ? <Box size={8} /> : <Link2 size={8} />}
                    </span>
                  )}
                </button>
                  )
                })}
                </div>
            </>
          )
        })()}
      </div>

        <span className="skill-open-hint" aria-hidden="true">
          ›
        </span>

        <div className="skill-actions-col">
          <button type="button" onClick={() => onEditTags(skill)} disabled={loading} aria-label={t('editTags')} title={t('editTags')}><Tag size={16} /></button>
          <button
            type="button"
            onClick={() => onUpdate(skill)}
            disabled={loading || !enabled || !updateable}
            aria-label={updateable ? t('update') : t('updateSourceUnavailable')}
            title={updateable ? t('update') : t('updateSourceUnavailable')}
          ><RefreshCw size={16} /></button>
          {onIgnore && (
            <button
              type="button"
              onClick={() => onIgnore(skill.id)}
              disabled={loading}
              aria-label={t('ignoreSkill', { defaultValue: '屏蔽' })}
              title={t('ignoreSkillTooltip', { defaultValue: '屏蔽（解除托管并保留软件副本）' })}
            >
              <ShieldOff size={16} />
            </button>
          )}
          <button
            type="button"
            onClick={() => onDelete(skill.id)}
            disabled={loading}
            aria-label={t('remove')}
            title={t('remove')}
          >
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      {contextMenu && (
        <ToolContextMenu
          state={contextMenu}
          onClose={() => setContextMenu(null)}
          onSyncWithMode={(s, tId, mode) => {
            onSyncToolWithMode?.(s, tId, mode)
          }}
          onUnsync={(s, tId) => {
            onUnsyncTool?.(s, tId)
          }}
          onPromoteToOwner={(s, tId, inst) => {
            onPromoteToolToOwner?.(s, tId, inst)
          }}
          onOpenFolder={(path) => {
            openPath(path).catch(() => {
              toast.error('无法打开目标目录')
            })
          }}
        />
      )}
    </div>
  )
}

export default memo(SkillCard)

