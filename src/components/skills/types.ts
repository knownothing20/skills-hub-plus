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

export type SyncMode = 'auto' | 'symlink' | 'junction' | 'copy'

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


