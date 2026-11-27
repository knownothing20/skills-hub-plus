export type OnboardingVariant = {
  tool: string
  name: string
  path: string
  fingerprint?: string | null
  is_link: boolean
  link_target?: string | null
  plugin_name?: string | null
  plugin_version?: string | null
  plugin_scope?: string | null
}

export type OnboardingGroup = {
  name: string
  variants: OnboardingVariant[]
  has_conflict: boolean
}

export type OnboardingPlan = {
  total_tools_scanned: number
  total_skills_found: number
  groups: OnboardingGroup[]
}

export type ToolOption = {
  id: string
  label: string
  avatar?: string | null
  supports_project_scope?: boolean
}

export type SyncMode = 'copy'

export type TagDto = {
  id: number
  name: string
}

export type TagWithCountDto = TagDto & {
  skill_count: number
  updated_at: number
}

export type ManagedSkill = {
  id: string
  name: string
  description?: string | null
  icon_data_url?: string | null
  brand_color?: string | null
  source_type: string
  source_ref?: string | null
  has_external_source: boolean
  central_path: string
  created_at: number
  updated_at: number
  /** 内容最后变更时间。区别于 updated_at：后者只随元数据操作变，不随内容编辑变。 */
  content_changed_at: number
  /** 内容是否真的被编辑过（首次建立基线的那条历史不算）。 */
  content_edited: boolean
  /**
   * 技能自己声明的版本号（`manifest.json` 或 `SKILL.md` frontmatter）。
   * 48 个技能里只有 8 个有，其余为 null —— 界面统一显示成 `v0`。
   */
  version?: string | null
  last_sync_at?: number | null
  enabled: boolean
  updateable: boolean
  status: string
  tags: TagDto[]
  targets: {
    tool: string
    scope: 'global' | 'project' | string
    project_path?: string | null
    mode: string
    status: string
    last_error?: string | null
    target_path: string
    synced_at?: number | null
    role?: string
    /** 多实例目录标识（账号 / Profile）；空串或未给出＝主实例 */
    instance?: string
  }[]
}

export type GitSkillCandidate = {
  name: string
  description?: string | null
  subpath: string
}

export type LocalSkillCandidate = {
  name: string
  description?: string | null
  subpath: string
  valid: boolean
  reason?: string | null
}

export type InstallResultDto = {
  skill_id: string
  name: string
  central_path: string
  content_hash?: string | null
}

export type ToolInfoDto = {
  key: string
  label: string
  avatar?: string | null
  installed: boolean
  enabled: boolean
  is_custom: boolean
  skills_dir: string
  project_skills_dir: string
  supports_project_scope: boolean
  sync_mode: SyncMode
}

export type ToolStatusDto = {
  tools: ToolInfoDto[]
  installed: string[]
  newly_installed: string[]
}

/** 复制副本的落后状态；verdict: behind | hand_edited | conflict */
export type StaleCopyStatusDto = {
  skill_id: string
  skill_name: string
  tool: string
  tool_label: string
  target_path: string
  verdict: 'behind' | 'hand_edited' | 'conflict'
}

/** 从软件自己的配置里读出的账号身份；登出后多数字段会被清空 */
export type AccountIdentityDto = {
  /** 真实账号名（豆包 `saman.last_user.name`）；登出后为空 */
  display_name: string
  /** 数字用户 ID；登出后为空 */
  user_id: string
  /** 租户内用户名；登出后可能仍保留 */
  tenant_username: string
  /** Chromium profile 显示名，可能是自动生成的 */
  profile_name: string
  /** user_id 非空即视为已登录 */
  signed_in: boolean
}

export type SkillRootInstanceDto = {
  tool: string
  /** 账号目录名（Default / Profile 4），单实例工具为空 */
  label: string
  /** 实例键：主实例为空串，其余等于 label；与 skill_targets.instance 对齐 */
  instance: string
  path: string
  skills: string[]
  is_primary: boolean
  exists: boolean
  account: AccountIdentityDto
}

export type SharedSkillDirDto = {
  path: string
  label: string
  readers: string[]
  exists: boolean
  exposed_skills: string[]
}

export type StaleMirrorDto = {
  skill_id: string
  name: string
  owner_path: string
  central_path: string
  verdict: 'behind' | 'hand_edited'
}

export type StartupStaleRefreshDto = {
  refreshed: number
  stale_mirrors: number
  remaining: number
  auto_refresh_enabled: boolean
}

export type CustomToolConfigDto = {
  key: string
  label: string
  avatar?: string | null
  skills_dir: string
  project_skills_dir?: string | null
  sync_mode: SyncMode
  enabled: boolean
}

export type ToolConfigDto = {
  disabled_builtin_tools: string[]
  custom_tools: CustomToolConfigDto[]
}

export type DiscoveryScanSourceDto = {
  key: string
  label: string
  path: string
  enabled: boolean
}

export type DiscoveryScanSettingsDto = {
  sources: DiscoveryScanSourceDto[]
  disabled_source_keys: string[]
}

export type UpdateResultDto = {
  skill_id: string
  name: string
  content_hash?: string | null
  source_revision?: string | null
  updated_targets: string[]
}

export type AutoUpdateConfigDto = {
  enabled: boolean
  interval_hours: number
  schedule_type: 'interval' | 'daily'
  interval_value: number
  interval_unit: 'minutes' | 'hours'
  daily_time: string
  local_skill_count: number
  protected_local_skill_count: number
  task_registered: boolean
  task_status_detail: string
  last_run_at?: number | null
  last_started_at?: number | null
  last_finished_at?: number | null
  last_status?: string | null
  last_error?: string | null
  last_checked: number
  last_updated: number
  last_failed: number
  progress: AutoUpdateProgressSnapshotDto
}

export type AutoUpdateRunResultDto = {
  checked: number
  updated: number
  skipped: number
  failed: number
  errors: string[]
  progress: AutoUpdateProgressSnapshotDto
}

export type GithubProxyConfigDto = {
  enabled: boolean
  port: number
  url: string
  auto_detected: boolean
}

export type AutoUpdateSkillProgressDto = {
  skill_id: string
  name: string
  reason?: string | null
}

export type AutoUpdateProgressSnapshotDto = {
  total: number
  succeeded: AutoUpdateSkillProgressDto[]
  skipped: AutoUpdateSkillProgressDto[]
  failed: AutoUpdateSkillProgressDto[]
  running?: AutoUpdateSkillProgressDto | null
  pending: AutoUpdateSkillProgressDto[]
}

export type FeaturedSkillDto = {
  slug: string
  name: string
  summary: string
  downloads: number
  stars: number
  source_url: string
}

export type OnlineSkillDto = {
  name: string
  installs: number
  source: string
  source_url: string
}

export type SkillFileEntry = {
  path: string
  size: number
}

export type BackupConfigDto = {
  enabled: boolean
  backup_dir: string
  last_backup_time: string | null
  last_backup_count: number | null
}

export type BackupManifestDto = {
  version: string
  backup_time: string
  skill_count: number
  central_path: string
}

export type ToolOptionWithCount = {
  id: string
  label: string
  avatar?: string | null
  count: number
}

export type BrokenTargetDto = {
  tool: string
  path: string
  reason: 'missing' | 'broken_link' | string
}

export type SkillHealthDto = {
  name: string
  enabled: boolean
  synced_tool_count: number
  broken: BrokenTargetDto[]
  last_modified: number
  idle: boolean
}

export type HealthSummaryDto = {
  total: number
  synced_skills: number
  broken_targets: number
  idle_skills: number
}

export type LibraryHealthReportDto = {
  summary: HealthSummaryDto
  skills: SkillHealthDto[]
}

export type LockTargetEntryDto = {
  tool: string
  scope: string
  target_path: string
}

export type LockSkillEntryDto = {
  name: string
  enabled: boolean
  tags: string[]
  source_ref?: string | null
  content_hash: string
  targets: LockTargetEntryDto[]
}

export type ExportLockResultDto = {
  path: string
  skill_count: number
}

export type LockDriftDto = {
  name: string
  lock_hash: string
  local_hash: string
}

export type LockDiffReportDto = {
  lock_path: string
  missing_here: LockSkillEntryDto[]
  extra_here: string[]
  drifted: LockDriftDto[]
  targets_changed: string[]
}


