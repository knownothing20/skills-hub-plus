import { memo, useCallback, useEffect, useMemo, useState } from 'react'
import {
  Archive,
  ArrowLeft,
  Database,
  ExternalLink,
  FileDown,
  FileJson,
  FileUp,
  FolderOpen,
  Github,
  Link2,
  Palette,
  Radar,
  RefreshCw,
  ShieldCheck,
  EyeOff,
} from 'lucide-react'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import { invokeTauri } from '../../hooks/useTauriInvoke'
import LockDiffModal from './modals/LockDiffModal'
import type {
  BackupConfigDto,
  BackupManifestDto,
  ExportLockResultDto,
  GithubProxyConfigDto,
  LockDiffReportDto,
} from './types'

const PROJECT_REPOSITORY_URL = 'https://github.com/mcncarl/skills-hub'

type SettingsPageProps = {
  isTauri: boolean
  language: string
  storagePath: string
  gitCacheTtlSecs: number
  themePreference: 'system' | 'light' | 'dark'
  defaultSyncMode: 'junction' | 'copy'
  githubProxyConfig: GithubProxyConfigDto
  discoveryScanEnabledCount: number
  discoveryScanSourceCount: number
  onToggleLanguage: () => void
  onThemeChange: (nextTheme: 'system' | 'light' | 'dark') => void
  onDefaultSyncModeChange: (nextMode: 'junction' | 'copy') => void
  onGitCacheTtlSecsChange: (nextSecs: number) => void
  onClearGitCacheNow: () => void
  onGithubProxyConfigChange: (enabled: boolean, port: number) => void
  onOpenDiscoveryScanSettings: () => void
  onBack: () => void
  t: TFunction
}

const SettingsPage = ({
  isTauri,
  language,
  storagePath,
  gitCacheTtlSecs,
  themePreference,
  defaultSyncMode,
  onToggleLanguage,
  onThemeChange,
  onDefaultSyncModeChange,
  onGitCacheTtlSecsChange,
  onClearGitCacheNow,
  githubProxyConfig,
  onGithubProxyConfigChange,
  discoveryScanEnabledCount,
  discoveryScanSourceCount,
  onOpenDiscoveryScanSettings,
  onBack,
  t,
}: SettingsPageProps) => {
  const [localGithubProxyPort, setLocalGithubProxyPort] = useState(
    String(githubProxyConfig.port),
  )
  useEffect(() => {
    setLocalGithubProxyPort(String(githubProxyConfig.port))
  }, [githubProxyConfig.port])

  const [appVersion, setAppVersion] = useState<string | null>(null)
  const versionText = useMemo(() => {
    if (!isTauri) return t('notAvailable')
    if (!appVersion) return t('unknown')
    return `v${appVersion}`
  }, [appVersion, isTauri, t])

  const loadAppVersion = useCallback(async () => {
    if (!isTauri) {
      setAppVersion(null)
      return
    }
    try {
      const { getVersion } = await import('@tauri-apps/api/app')
      const v = await getVersion()
      setAppVersion(v)
    } catch {
      setAppVersion(null)
    }
  }, [isTauri])

  useEffect(() => {
    void loadAppVersion()
  }, [loadAppVersion])

  const [backupConfig, setBackupConfig] = useState<BackupConfigDto>({
    enabled: true,
    backup_dir: 'D:\\GitHub\\skill-hub\\backup',
    last_backup_time: null,
    last_backup_count: null,
  })
  const [isBackingUp, setIsBackingUp] = useState(false)
  const [isRestoring, setIsRestoring] = useState(false)

  const [ignoredSkills, setIgnoredSkills] = useState<string[]>([])
  const loadIgnoredSkills = useCallback(async () => {
    if (!isTauri) return
    try {
      const list = await invokeTauri<string[]>('get_ignored_skills')
      setIgnoredSkills(list ?? [])
    } catch {
      setIgnoredSkills([])
    }
  }, [isTauri])

  useEffect(() => {
    void loadIgnoredSkills()
  }, [loadIgnoredSkills])

  const handleUnignore = useCallback(
    async (name: string) => {
      try {
        await invokeTauri('unignore_skill', { name })
        toast.success(t('unignoreSuccess'))
        await loadIgnoredSkills()
      } catch (err) {
        toast.error(err instanceof Error ? err.message : String(err))
      }
    },
    [loadIgnoredSkills, t],
  )

  useEffect(() => {
    if (isTauri) {
      void (async () => {
        try {
          const { invoke } = await import('@tauri-apps/api/core')
          const cfg = await invoke<BackupConfigDto>('get_backup_config')
          setBackupConfig(cfg)
        } catch (e) {
          console.error('Failed to get backup config:', e)
        }
      })()
    }
  }, [isTauri])

  const handleToggleAutoBackup = async (enabled: boolean) => {
    const updated = { ...backupConfig, enabled }
    setBackupConfig(updated)
    if (isTauri) {
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        await invoke('save_backup_config', { config: updated })
      } catch (e) {
        console.error(e)
      }
    }
  }

  const handleBackupDirChange = async (dir: string) => {
    const updated = { ...backupConfig, backup_dir: dir }
    setBackupConfig(updated)
    if (isTauri) {
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        await invoke('save_backup_config', { config: updated })
      } catch (e) {
        console.error(e)
      }
    }
  }

  const handleSelectBackupDir = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择备份存储目录',
      })
      if (selected && typeof selected === 'string') {
        await handleBackupDirChange(selected)
      }
    } catch (e) {
      console.error(e)
    }
  }

  const handleCreateBackup = async () => {
    if (!isTauri) return
    setIsBackingUp(true)
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const manifest = await invoke<BackupManifestDto>('create_backup_now', {
        customDir: backupConfig.backup_dir,
      })
      setBackupConfig((prev) => ({
        ...prev,
        last_backup_time: manifest.backup_time,
        last_backup_count: manifest.skill_count,
      }))
      toast.success(
        t('backupSuccess', {
          count: manifest.skill_count,
          path: backupConfig.backup_dir,
        }),
      )
    } catch (err) {
      toast.error(t('backupFailed', { error: String(err) }))
    } finally {
      setIsBackingUp(false)
    }
  }

  const handleRestoreBackup = async () => {
    if (!isTauri) return
    const confirmed = window.confirm(t('restoreConfirm'))
    if (!confirmed) return

    setIsRestoring(true)
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const count = await invoke<number>('restore_backup_now', {
        sourceDir: backupConfig.backup_dir,
      })
      toast.success(t('restoreSuccess', { count }))
    } catch (err) {
      toast.error(t('restoreFailed', { error: String(err) }))
    } finally {
      setIsRestoring(false)
    }
  }

  const [isExportingLock, setIsExportingLock] = useState(false)
  const [isComparingLock, setIsComparingLock] = useState(false)
  const [lockReport, setLockReport] = useState<LockDiffReportDto | null>(null)
  const [lockReportOpen, setLockReportOpen] = useState(false)

  const handleExportLock = async () => {
    if (!isTauri) return
    let target: string | null = null
    try {
      const { save } = await import('@tauri-apps/plugin-dialog')
      target = await save({
        title: t('lock.exportDialogTitle'),
        defaultPath: 'skills.lock',
      })
    } catch (err) {
      toast.error(t('lock.exportFailed', { error: String(err) }))
      return
    }
    if (!target) return

    setIsExportingLock(true)
    try {
      const result = await invokeTauri<ExportLockResultDto>('export_skills_lock_cmd', {
        path: target,
      })
      toast.success(t('lock.exportSuccess', { count: result.skill_count, path: result.path }))
    } catch (err) {
      toast.error(t('lock.exportFailed', { error: String(err) }))
    } finally {
      setIsExportingLock(false)
    }
  }

  const handleImportLock = async () => {
    if (!isTauri) return
    let selected: string | null = null
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const picked = await open({
        multiple: false,
        title: t('lock.importDialogTitle'),
      })
      if (picked && typeof picked === 'string') {
        selected = picked
      }
    } catch (err) {
      toast.error(t('lock.importFailed', { error: String(err) }))
      return
    }
    if (!selected) return

    setIsComparingLock(true)
    try {
      const report = await invokeTauri<LockDiffReportDto>('import_skills_lock_cmd', {
        path: selected,
      })
      setLockReport(report)
      setLockReportOpen(true)
    } catch (err) {
      toast.error(t('lock.importFailed', { error: String(err) }))
    } finally {
      setIsComparingLock(false)
    }
  }

  const handleOpenProject = useCallback(async () => {
    try {
      if (isTauri) {
        await openUrl(PROJECT_REPOSITORY_URL)
      } else {
        window.open(PROJECT_REPOSITORY_URL, '_blank', 'noopener,noreferrer')
      }
    } catch {
      toast.error(t('projectLink.openFailed'))
    }
  }, [isTauri, t])

  return (
    <div className="settings-page">
      <div className="settings-shell">
        <div className="settings-hero">
          <div className="settings-hero-main">
            <div className="settings-title-row">
              <button className="detail-back-btn settings-back" type="button" onClick={onBack}>
                <ArrowLeft size={16} />
                {t('detail.back')}
              </button>
              <div className="settings-title-copy">
                <h1>{t('settings')}</h1>
                <p>{t('settingsPageSubtitle')}</p>
              </div>
            </div>
          </div>
          <div className="settings-hero-summary" aria-label={t('settingsSummary')}>
            <div className="settings-summary-item">
              <span>{t('interfaceLanguage')}</span>
              <strong>{t(`languageOptions.${language}`)}</strong>
            </div>
            <div className="settings-summary-item">
              <span>{t('themeMode')}</span>
              <strong>{t(`themeOptions.${themePreference}`)}</strong>
            </div>
            <div className="settings-summary-item">
              <span>{t('defaultSyncMode')}</span>
              <strong>
                {defaultSyncMode === 'junction'
                  ? t('syncModeJunction')
                  : t('syncModeCopy')}
              </strong>
            </div>
            <div className="settings-summary-item">
              <span>{t('appVersion')}</span>
              <strong>{versionText}</strong>
            </div>
          </div>
        </div>

        <div className="settings-grid">
          <div className="settings-column">
            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <Palette size={18} />
                </span>
                <div>
                  <h2>{t('settingsSectionAppearance')}</h2>
                  <p>{t('settingsSectionAppearanceDesc')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-field">
                  <label className="settings-label" htmlFor="settings-language">
                    {t('interfaceLanguage')}
                  </label>
                  <div className="settings-select-wrap">
                    <select
                      id="settings-language"
                      className="settings-select"
                      value={language}
                      onChange={(event) => {
                        if (event.target.value !== language) {
                          onToggleLanguage()
                        }
                      }}
                    >
                      <option value="en">{t('languageOptions.en')}</option>
                      <option value="zh">{t('languageOptions.zh')}</option>
                    </select>
                    <svg
                      className="settings-select-caret"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      aria-hidden="true"
                    >
                      <path d="M6 9l6 6 6-6" />
                    </svg>
                  </div>
                </div>

                <div className="settings-field">
                  <label className="settings-label" id="settings-theme-label">
                    {t('themeMode')}
                  </label>
                  <div className="settings-theme-options" role="group" aria-labelledby="settings-theme-label">
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        themePreference === 'system' ? 'active' : ''
                      }`}
                      aria-pressed={themePreference === 'system'}
                      onClick={() => onThemeChange('system')}
                    >
                      {t('themeOptions.system')}
                    </button>
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        themePreference === 'light' ? 'active' : ''
                      }`}
                      aria-pressed={themePreference === 'light'}
                      onClick={() => onThemeChange('light')}
                    >
                      {t('themeOptions.light')}
                    </button>
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        themePreference === 'dark' ? 'active' : ''
                      }`}
                      aria-pressed={themePreference === 'dark'}
                      onClick={() => onThemeChange('dark')}
                    >
                      {t('themeOptions.dark')}
                    </button>
                  </div>
                </div>
              </div>
            </section>

            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <Radar size={18} />
                </span>
                <div>
                  <h2>{t('discoveryScan.settingsTitle')}</h2>
                  <p>{t('discoveryScan.settingsDescription')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-project-row discovery-settings-row">
                  <div className="settings-item-info">
                    <div className="settings-item-title">{t('discoveryScan.sources')}</div>
                    <div className="settings-item-desc">
                      {t('discoveryScan.enabledCount', {
                        enabled: discoveryScanEnabledCount,
                        total: discoveryScanSourceCount,
                      })}
                    </div>
                  </div>
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    onClick={onOpenDiscoveryScanSettings}
                  >
                    {t('discoveryScan.manage')}
                  </button>
                </div>
              </div>
            </section>

            <section className="settings-card">
            <div className="settings-card-head">
              <span className="settings-card-icon">
                <Database size={18} />
              </span>
              <div>
                <h2>{t('settingsSectionStorage')}</h2>
                <p>{t('settingsSectionStorageDesc')}</p>
              </div>
            </div>
            <div className="settings-card-body">
              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-storage">
                  {t('skillsStoragePath')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-storage"
                    className="settings-input mono"
                    value={storagePath}
                    readOnly
                  />
                </div>
                <div className="settings-helper">{t('skillsStorageHint')}</div>
              </div>

              <div className="settings-field">
                <label className="settings-label">
                  {t('gitCacheCleanupDays')}
                </label>
                <div className="settings-input-row">
                  <button
                    className="btn btn-secondary settings-browse"
                    type="button"
                    onClick={onClearGitCacheNow}
                  >
                    {t('cleanNow')}
                  </button>
                </div>
                <div className="settings-helper">{t('gitCacheCleanupHint')}</div>
              </div>

              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-git-cache-ttl">
                  {t('gitCacheTtlSecs')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-git-cache-ttl"
                    className="settings-input"
                    type="number"
                    min={0}
                    max={3600}
                    step={1}
                    value={gitCacheTtlSecs}
                    onChange={(event) => {
                      const next = Number(event.target.value)
                      if (!Number.isNaN(next)) {
                        onGitCacheTtlSecsChange(next)
                      }
                    }}
                  />
                </div>
                <div className="settings-helper">{t('gitCacheTtlHint')}</div>
              </div>
            </div>
            </section>

            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <Archive size={18} />
                </span>
                <div>
                  <h2>{t('backupSectionTitle')}</h2>
                  <p>{t('backupSectionDesc')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-project-row" style={{ marginBottom: '16px' }}>
                  <div className="settings-item-info">
                    <div className="settings-item-title">{t('backupAutoEnable')}</div>
                    <div className="settings-item-desc">{t('backupAutoEnableHint')}</div>
                  </div>
                  <button
                    type="button"
                    className={`settings-toggle${backupConfig.enabled ? ' checked' : ''}`}
                    aria-pressed={backupConfig.enabled}
                    onClick={() => void handleToggleAutoBackup(!backupConfig.enabled)}
                  >
                    <span className="settings-toggle-knob" />
                  </button>
                </div>

                <div className="settings-field">
                  <label className="settings-label" htmlFor="settings-backup-dir">
                    {t('backupDirPath')}
                  </label>
                  <div className="settings-input-row" style={{ display: 'flex', gap: '8px' }}>
                    <input
                      id="settings-backup-dir"
                      className="settings-input mono"
                      value={backupConfig.backup_dir}
                      onChange={(e) => void handleBackupDirChange(e.target.value)}
                      style={{ flex: 1 }}
                    />
                    <button
                      className="btn btn-secondary settings-browse"
                      type="button"
                      onClick={() => void handleSelectBackupDir()}
                      title={t('backupSelectDir')}
                    >
                      <FolderOpen size={15} style={{ marginRight: '4px' }} />
                      {t('backupSelectDir')}
                    </button>
                  </div>
                  <div className="settings-helper">
                    {backupConfig.last_backup_time
                      ? t('backupStatusLast', {
                          time: backupConfig.last_backup_time,
                          count: backupConfig.last_backup_count ?? 0,
                        })
                      : t('backupStatusNever')}
                  </div>
                </div>

                <div className="settings-field" style={{ marginTop: '16px', display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
                  <button
                    className="btn btn-primary"
                    type="button"
                    disabled={isBackingUp || isRestoring}
                    onClick={() => void handleCreateBackup()}
                  >
                    <RefreshCw size={14} className={isBackingUp ? 'spin' : ''} style={{ marginRight: '6px' }} />
                    {isBackingUp ? '正在备份...' : t('backupNowBtn')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    disabled={isBackingUp || isRestoring}
                    onClick={() => void handleRestoreBackup()}
                    style={{ borderColor: 'var(--color-border-warning, #f59e0b)' }}
                  >
                    <Archive size={14} style={{ marginRight: '6px' }} />
                    {isRestoring ? '正在恢复...' : t('backupRestoreBtn')}
                  </button>
                </div>
              </div>
            </section>

            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <FileJson size={18} />
                </span>
                <div>
                  <h2>{t('lock.title')}</h2>
                  <p>{t('lock.description')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-field" style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
                  <button
                    className="btn btn-primary"
                    type="button"
                    disabled={isExportingLock || isComparingLock}
                    onClick={() => void handleExportLock()}
                  >
                    <FileDown size={14} style={{ marginRight: '6px' }} />
                    {isExportingLock ? t('lock.exporting') : t('lock.exportBtn')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    disabled={isExportingLock || isComparingLock}
                    onClick={() => void handleImportLock()}
                  >
                    <FileUp size={14} style={{ marginRight: '6px' }} />
                    {isComparingLock ? t('lock.comparing') : t('lock.importBtn')}
                  </button>
                </div>
                <div className="settings-helper">{t('lock.reportHint')}</div>
              </div>
            </section>
          </div>

          <div className="settings-column">
            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <Link2 size={18} />
                </span>
                <div>
                  <h2>{t('syncModeSectionTitle')}</h2>
                  <p>{t('syncModeSectionDesc')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-field">
                  <label className="settings-label" id="settings-sync-mode-label">
                    {t('defaultSyncMode')}
                  </label>
                  <div className="settings-theme-options" role="group" aria-labelledby="settings-sync-mode-label">
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        defaultSyncMode === 'junction' ? 'active' : ''
                      }`}
                      aria-pressed={defaultSyncMode === 'junction'}
                      onClick={() => onDefaultSyncModeChange('junction')}
                    >
                      🔗 {t('syncModeJunction')}
                    </button>
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        defaultSyncMode === 'copy' ? 'active' : ''
                      }`}
                      aria-pressed={defaultSyncMode === 'copy'}
                      onClick={() => onDefaultSyncModeChange('copy')}
                    >
                      📦 {t('syncModeCopy')}
                    </button>
                  </div>
                  <div className="settings-helper" style={{ marginTop: '8px' }}>
                    {defaultSyncMode === 'junction'
                      ? t('syncModeJunctionDesc')
                      : t('syncModeCopyDesc')}
                  </div>
                </div>
              </div>
            </section>

            <section className="settings-card">
            <div className="settings-card-head">
              <span className="settings-card-icon">
                <Github size={18} />
              </span>
              <div>
                <h2>{t('settingsSectionNetwork')}</h2>
                <p>{t('settingsSectionNetworkDesc')}</p>
              </div>
            </div>
            <div className="settings-card-body">
              <div className="settings-project-row">
                <div className="settings-item-info">
                  <div className="settings-item-title">{t('projectLink.title')}</div>
                  <div className="settings-item-desc">{t('projectLink.description')}</div>
                </div>
                <button
                  className="btn btn-secondary btn-sm settings-project-link"
                  type="button"
                  onClick={() => void handleOpenProject()}
                  aria-label={t('projectLink.open')}
                >
                  {t('projectLink.view')}
                  <ExternalLink size={14} />
                </button>
              </div>
              <div className="settings-field">
                <div className="settings-item">
                  <div className="settings-item-info">
                    <div className="settings-item-title">{t('networkProxy')}</div>
                    <div className="settings-item-desc">{t('networkProxyHint')}</div>
                  </div>
                  <button
                    type="button"
                    className={`settings-toggle${githubProxyConfig.enabled ? ' checked' : ''}`}
                    aria-pressed={githubProxyConfig.enabled}
                    onClick={() => {
                      const nextPort = Number(localGithubProxyPort)
                      onGithubProxyConfigChange(
                        !githubProxyConfig.enabled,
                        Number.isNaN(nextPort) ? githubProxyConfig.port : nextPort,
                      )
                    }}
                  >
                    <span className="settings-toggle-knob" />
                  </button>
                </div>
                <label className="settings-label" htmlFor="settings-github-proxy-port">
                  {t('networkProxyPort')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-github-proxy-port"
                    className="settings-input mono"
                    type="number"
                    min={1}
                    max={65535}
                    step={1}
                    value={localGithubProxyPort}
                    onChange={(e) => setLocalGithubProxyPort(e.target.value)}
                    onBlur={() => {
                      const nextPort = Number(localGithubProxyPort)
                      if (
                        githubProxyConfig.enabled &&
                        !Number.isNaN(nextPort) &&
                        nextPort !== githubProxyConfig.port
                      ) {
                        onGithubProxyConfigChange(true, nextPort)
                      }
                    }}
                  />
                </div>
                <div className="settings-helper">
                  {githubProxyConfig.auto_detected
                    ? t('networkProxyAutoDetected')
                    : t('networkProxyPortHint')}
                </div>
              </div>
            </div>
            </section>

            <section className="settings-card">
            <div className="settings-card-head">
                <span className="settings-card-icon">
                  <ShieldCheck size={18} />
                </span>
                <div>
                  <h2>{t('settingsSectionUpdates')}</h2>
                  <p>{t('settingsSectionUpdatesDesc')}</p>
                </div>
            </div>
            <div className="settings-card-body">
              <div className="settings-version-row">
                <div>
                  <span className="settings-version-label">{t('appVersion')}</span>
                  <span className="settings-version-text">{versionText}</span>
                </div>
              </div>
              <div className="settings-helper">{t('safeBuildUpdatePolicy')}</div>
            </div>
            </section>
            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <EyeOff size={18} />
                </span>
                <div>
                  <h2>{t('ignoredSkillsTitle')}</h2>
                  <p>{t('ignoredSkillsDesc')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                {ignoredSkills.length === 0 ? (
                  <div className="settings-helper">{t('noIgnoredSkills')}</div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    {ignoredSkills.map((name) => (
                      <div
                        key={name}
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'space-between',
                          padding: '8px 12px',
                          borderRadius: '6px',
                          backgroundColor: 'var(--bg-secondary)',
                          border: '1px solid var(--border-color)',
                        }}
                      >
                        <span style={{ fontWeight: 500 }}>{name}</span>
                        <button
                          type="button"
                          className="btn btn-secondary"
                          style={{ padding: '2px 8px', fontSize: '12px' }}
                          onClick={() => void handleUnignore(name)}
                        >
                          {t('unignoreSkill')}
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </section>
          </div>
        </div>
      </div>

      <LockDiffModal
        open={lockReportOpen}
        report={lockReport}
        onRequestClose={() => setLockReportOpen(false)}
        t={t}
      />
    </div>
  )
}

export default memo(SettingsPage)
