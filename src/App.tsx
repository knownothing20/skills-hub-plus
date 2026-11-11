import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { RefreshCw } from 'lucide-react'
import './App.css'
import './figma.css'
import { useTranslation } from 'react-i18next'
import { Toaster, toast } from 'sonner'
import {
  isTauri,
  invokeTauri,
  useFormatErrorMessage,
  useShowActionErrors,
  useFormatRelative,
  getGithubInfo,
  getSkillSourceLabel,
} from './hooks/useTauriInvoke'
import { useAppSettings } from './hooks/useAppSettings'
import ExplorePage from './components/skills/ExplorePage'
import FilterBar from './components/skills/FilterBar'
import SkillDetailView from './components/skills/SkillDetailView'
import { openPath } from '@tauri-apps/plugin-opener'
import { listen } from '@tauri-apps/api/event'
import CentralLibraryView from './components/skills/CentralLibraryView'
import TitleBar from './components/skills/TitleBar'
import Sidebar from './components/skills/Sidebar'
import LoadingOverlay from './components/skills/LoadingOverlay'
import SkillsList from './components/skills/SkillsList'
import TagsPage from './components/skills/TagsPage'
import AddSkillModal from './components/skills/modals/AddSkillModal'
import BulkDeleteModal from './components/skills/modals/BulkDeleteModal'
import BulkIgnoreModal from './components/skills/modals/BulkIgnoreModal'
import BulkSyncModal from './components/skills/modals/BulkSyncModal'
import BulkTagsModal from './components/skills/modals/BulkTagsModal'
import DeleteModal from './components/skills/modals/DeleteModal'
import IgnoreModal from './components/skills/modals/IgnoreModal'
import DiscoveryScanModal from './components/skills/modals/DiscoveryScanModal'
import EditSkillTagsModal from './components/skills/modals/EditSkillTagsModal'
import GitPickModal from './components/skills/modals/GitPickModal'
import LocalPickModal from './components/skills/modals/LocalPickModal'
import ImportModal from './components/skills/modals/ImportModal'
import NewToolsModal from './components/skills/modals/NewToolsModal'
import RenameTagModal from './components/skills/modals/RenameTagModal'
import ScopeSyncModal from './components/skills/modals/ScopeSyncModal'
import SharedDirModal from './components/skills/modals/SharedDirModal'
import ToggleToolConfirmModal from './components/skills/modals/ToggleToolConfirmModal'
import SettingsPage from './components/skills/SettingsPage'
import ToolsPage from './components/skills/ToolsPage'
import WindowResizeHandles from './components/WindowResizeHandles'
import { useDiscoveryScan } from './hooks/useDiscoveryScan'
import { useBulkOperations } from './hooks/useBulkOperations'
import { useSkillFilters } from './hooks/useSkillFilters'
import { useTagOperations } from './hooks/useTagOperations'
import { useSkillImport } from './hooks/useSkillImport'
import { useToolSync } from './hooks/useToolSync'
import { useSkillScope } from './hooks/useSkillScope'
import { useSkillLifecycle } from './hooks/useSkillLifecycle'
import { useSystemSettings } from './hooks/useSystemSettings'
import { useLibraryHealth } from './hooks/useLibraryHealth'
import { useStaleCopies } from './hooks/useStaleCopies'
import { useImportPlan } from './hooks/useImportPlan'
import { useInstallScopeTargets } from './hooks/useInstallScopeTargets'
import { useExploreSkills } from './hooks/useExploreSkills'
import {
  getSkillSyncState,
  getToolSyncState,
} from './components/skills/skillSyncStatus'
import type {
  ManagedSkill,
  SharedSkillDirDto,
  SkillRootInstanceDto,
  SkillHealthDto,
  ToolOption,
  ToolStatusDto,
} from './components/skills/types'


type ActiveView = 'myskills' | 'explore' | 'detail' | 'settings' | 'manage' | 'library'
type ManagementTab = 'tags' | 'tools'

function App() {
  const { t } = useTranslation()
  const {
    language,
    toggleLanguage,
    themePreference,
    setThemePreference,
    defaultSyncMode,
    skillViewMode,
    setSkillViewMode,
    sidebarCollapsed,
    setSidebarCollapsed,
  } = useAppSettings()
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [actionMessage, setActionMessage] = useState<string | null>(null)
  const [successToastMessage, setSuccessToastMessage] = useState<string | null>(
    null,
  )
  const [managedSkills, setManagedSkills] = useState<ManagedSkill[]>([])
  const [sharedSkillDirs, setSharedSkillDirs] = useState<SharedSkillDirDto[]>([])
  // 每个工具实际有几个账号目录：卡片据此在图标右下角显示覆盖数
  const [toolInstanceTotals, setToolInstanceTotals] = useState<Record<string, number>>({})
  const [loadingStartAt, setLoadingStartAt] = useState<number | null>(null)
  const [toolStatus, setToolStatus] = useState<ToolStatusDto | null>(null)
  const [showNewToolsModal, setShowNewToolsModal] = useState(false)
  const [activeView, setActiveView] = useState<ActiveView>('myskills')
  const [managementTab, setManagementTab] = useState<ManagementTab>('tags')
  const [detailSkill, setDetailSkill] = useState<ManagedSkill | null>(null)
  const { libraryHealth, refreshLibraryHealth } = useLibraryHealth()
  const {
    staleStatuses,
    staleBySkillTool,
    staleCount,
    staleMirrors,
    staleMirrorCount,
    refreshStaleMirrors,
    autoRefreshEnabled,
    setAutoRefresh: setStaleAutoRefresh,
    runStartupRefresh,
    refreshOne: refreshStaleOne,
    refreshStaleStatus,
  } = useStaleCopies()

  const handleAutoRefreshStaleChange = useCallback(
    async (next: boolean) => {
      try {
        await setStaleAutoRefresh(next)
        toast.success(
          next ? t('stale.autoRefreshEnabledToast') : t('stale.autoRefreshDisabledToast'),
          { duration: 3200 },
        )
      } catch (err) {
        toast.error(String(err), { duration: 3200 })
      }
    },
    [setStaleAutoRefresh, t],
  )
  const {
    featuredSkills,
    featuredLoading,
    exploreFilter,
    searchResults,
    searchLoading,
    loadFeaturedSkills,
    handleExploreFilterChange,
  } = useExploreSkills()

  const formatErrorMessage = useFormatErrorMessage()
  const showActionErrors = useShowActionErrors()
  const isSkillNameTaken = useCallback(
    (name: string) =>
      managedSkills.some((skill) => skill.name.toLowerCase() === name.toLowerCase()),
    [managedSkills],
  )

  const formatRelative = useFormatRelative()



  const loadManagedSkills = useCallback(async () => {
    try {
      const result = await invokeTauri<ManagedSkill[]>('get_managed_skills')
      setManagedSkills(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [invokeTauri])

  // 用原件刷新母库镜像
  const handleRefreshMirror = useCallback(
    async (skillId: string) => {
      try {
        await invokeTauri('refresh_skill_mirror', { skillId })
        toast.success('母库已更新为原件内容')
        await loadManagedSkills()
        void refreshStaleMirrors()
        void refreshLibraryHealth()
      } catch (err) {
        toast.error(`更新母库失败: ${err instanceof Error ? err.message : String(err)}`)
      }
    },
    [invokeTauri, loadManagedSkills, refreshLibraryHealth, refreshStaleMirrors],
  )

  // 手动刷新：列表、健康栏、落后状态、版本字母一起重读
  const refreshAll = useCallback(async () => {
    await loadManagedSkills()
    void refreshLibraryHealth()
    void refreshStaleStatus()
    invokeTauri<Record<string, Record<string, string>>>('get_peer_version_map')
      .then((map) => setPeerVersionLabels(map || {}))
      .catch(() => {})
  }, [invokeTauri, loadManagedSkills, refreshLibraryHealth, refreshStaleStatus])

  // 后台同步完副本后会让界面重新拉一次数据
  useEffect(() => {
    let unlisten: (() => void) | undefined
    listen('skills-updated', () => {
      void loadManagedSkills()
    })
      .then((fn) => {
        unlisten = fn
      })
      .catch(() => {})
    return () => {
      unlisten?.()
    }
  }, [loadManagedSkills])

  // F5 / Ctrl+R 刷新：窗口里没有浏览器地址栏，得自己接
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const isRefresh =
        e.key === 'F5' || ((e.ctrlKey || e.metaKey) && (e.key === 'r' || e.key === 'R'))
      if (!isRefresh) return
      e.preventDefault()
      void refreshAll()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [refreshAll])

  // 各副本的版本字母：列表渲染完再异步算，不阻塞首屏
  const [peerVersionLabels, setPeerVersionLabels] = useState<
    Record<string, Record<string, string>>
  >({})
  useEffect(() => {
    let cancelled = false
    invokeTauri<Record<string, Record<string, string>>>('get_peer_version_map')
      .then((map) => {
        if (!cancelled) setPeerVersionLabels(map || {})
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [managedSkills])

  // 有操作时告诉后端「人还在」，用于界面隐藏后的闲置回收计时（30 秒最多上报一次）
  useEffect(() => {
    let last = 0
    const ping = () => {
      const now = Date.now()
      if (now - last < 30_000) return
      last = now
      void invokeTauri('touch_activity').catch(() => {})
    }
    window.addEventListener('mousemove', ping, { passive: true })
    window.addEventListener('mousedown', ping)
    window.addEventListener('keydown', ping)
    window.addEventListener('wheel', ping, { passive: true })
    return () => {
      window.removeEventListener('mousemove', ping)
      window.removeEventListener('mousedown', ping)
      window.removeEventListener('keydown', ping)
      window.removeEventListener('wheel', ping)
    }
  }, [invokeTauri])

  useEffect(() => {
    if (!successToastMessage) return
    toast.success(successToastMessage, { duration: 1800 })
    setSuccessToastMessage(null)
  }, [successToastMessage])

  useEffect(() => {
    if (!error) return
    const msg = formatErrorMessage(error)
    if (msg) toast.error(msg, { duration: 2600 })
    setError(null)
    setActionMessage(null)
  }, [error, formatErrorMessage])

  // 拦截全局默认浏览器右键菜单（防止出现网页级图片另存为等菜单），保留输入框文字编辑
  useEffect(() => {
    const handleContextMenu = (e: MouseEvent) => {
      try {
        const target = e.target as HTMLElement | null
        if (!target) return
        const isEditable =
          target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          (typeof target.closest === 'function' &&
            Boolean(target.closest('input, textarea, [contenteditable="true"]')))
        if (!isEditable) {
          e.preventDefault()
        }
      } catch {
        // 安全拦截，不抛异常
      }
    }
    window.addEventListener('contextmenu', handleContextMenu)
    return () => window.removeEventListener('contextmenu', handleContextMenu)
  }, [])

  const toolInfos = useMemo(() => toolStatus?.tools ?? [], [toolStatus])
  const enabledToolInfos = useMemo(
    () => toolInfos.filter((info) => info.enabled),
    [toolInfos],
  )

  const tools: ToolOption[] = useMemo(() => {
    return enabledToolInfos.map((info) => ({
      id: info.key,
      // Prefer i18n label if present; fallback to backend label.
      label: t(`tools.${info.key}`, { defaultValue: info.label }),
      avatar: info.avatar,
      supports_project_scope: info.supports_project_scope,
      supports_junction: info.supports_junction ?? true,
      supports_copy: info.supports_copy ?? true,
    }))
  }, [t, enabledToolInfos])

  const toolLabelById = useMemo(() => {
    const out: Record<string, string> = {}
    for (const tool of tools) out[tool.id] = tool.label
    return out
  }, [tools])

  const skillHealthByName = useMemo(() => {
    const out: Record<string, SkillHealthDto> = {}
    for (const skill of libraryHealth?.skills ?? []) out[skill.name] = skill
    return out
  }, [libraryHealth])

  // 共享目录（~/.agents/skills 这类开放标准位置）与其读取者：用于标注
  // “这个技能不只属于某一个工具”，以及展示无需链接即可用的工具图标。
  useEffect(() => {
    if (!isTauri) return
    let cancelled = false
    void (async () => {
      try {
        const dirs = await invokeTauri<SharedSkillDirDto[]>('get_shared_skill_dirs')
        if (!cancelled) setSharedSkillDirs(dirs)
      } catch (err) {
        console.warn(err)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [isTauri, managedSkills])

  useEffect(() => {
    if (!isTauri) return
    let cancelled = false
    void (async () => {
      try {
        const rows = await invokeTauri<SkillRootInstanceDto[]>(
          'get_tool_root_instances',
        )
        if (cancelled) return
        const totals: Record<string, number> = {}
        for (const row of rows) {
          if (row.exists) totals[row.tool] = (totals[row.tool] ?? 0) + 1
        }
        setToolInstanceTotals(totals)
      } catch (err) {
        console.warn(err)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [isTauri, managedSkills])

  // Keep health badges fresh after every managed-skills reload (sync, delete,
  // import, ... all end with loadManagedSkills).
  useEffect(() => {
    if (!isTauri) return
    void refreshLibraryHealth()
    void refreshStaleStatus()
  }, [isTauri, managedSkills, refreshLibraryHealth, refreshStaleStatus])

  const handleHealthBarClick = useCallback(() => {
    if (!libraryHealth) return
    const brokenSkills = libraryHealth.skills.filter(
      (skill) => skill.broken.length > 0,
    )
    if (brokenSkills.length === 0) {
      void refreshLibraryHealth()
      return
    }
    const lines = brokenSkills.slice(0, 8).map((skill) =>
      `${skill.name}: ${skill.broken
        .map(
          (broken) =>
            `${toolLabelById[broken.tool] ?? t(`tools.${broken.tool}`, { defaultValue: broken.tool })} · ${t(
              `health.reason.${broken.reason}`,
              { defaultValue: broken.reason },
            )}`,
        )
        .join('; ')}`,
    )
    const more =
      brokenSkills.length > 8
        ? `\n+${brokenSkills.length - 8}`
        : ''
    toast.error(
      t('health.brokenDetailsTitle', {
        count: libraryHealth.summary.broken_targets,
      }),
      {
        description: `${lines.join('\n')}${more}`,
        duration: 8000,
      },
    )
  }, [libraryHealth, refreshLibraryHealth, t, toolLabelById])

  const handleStaleCountClick = useCallback(() => {
    if (staleStatuses.length === 0) {
      void refreshStaleStatus()
      return
    }
    const lines = staleStatuses
      .slice(0, 8)
      .map(
        (status) =>
          `${status.skill_name}: ${status.tool_label} · ${t(`stale.verdict.${status.verdict}`)}`,
      )
    const more =
      staleStatuses.length > 8 ? `\n+${staleStatuses.length - 8}` : ''
    toast.warning(t('stale.detailsTitle', { count: staleStatuses.length }), {
      description: `${lines.join('\n')}${more}`,
      duration: 8000,
    })
  }, [staleStatuses, refreshStaleStatus, t])

  const sharedToolIdsByToolId = useMemo(() => {
    // toolId -> all toolIds that share the same skills_dir.
    const byDir: Record<string, string[]> = {}
    for (const info of enabledToolInfos) {
      const dir = info.skills_dir
      if (!byDir[dir]) byDir[dir] = []
      byDir[dir].push(info.key)
    }
    const out: Record<string, string[]> = {}
    for (const dir of Object.keys(byDir)) {
      const ids = byDir[dir]
      if (ids.length <= 1) continue
      for (const id of ids) out[id] = ids
    }
    return out
  }, [enabledToolInfos])

  const sharedProjectToolIdsByToolId = useMemo(() => {
    const byDir: Record<string, string[]> = {}
    for (const info of enabledToolInfos) {
      const dir = info.project_skills_dir
      if (!byDir[dir]) byDir[dir] = []
      byDir[dir].push(info.key)
    }
    const out: Record<string, string[]> = {}
    for (const ids of Object.values(byDir)) {
      if (ids.length <= 1) continue
      for (const id of ids) out[id] = ids
    }
    return out
  }, [enabledToolInfos])

  const uniqueToolIdsBySkillsDir = useCallback(
    (toolIds: string[]) => {
      // Preserve UI order (tools array order), de-dupe by skills_dir.
      const wanted = new Set(toolIds)
      const seen = new Set<string>()
      const out: string[] = []
      for (const tool of enabledToolInfos) {
        if (!wanted.has(tool.key)) continue
        if (seen.has(tool.skills_dir)) continue
        seen.add(tool.skills_dir)
        out.push(tool.key)
      }
      return out
    },
    [enabledToolInfos],
  )

  const uniqueToolIdsByProjectSkillsDir = useCallback(
    (toolIds: string[]) => {
      const wanted = new Set(toolIds)
      const seen = new Set<string>()
      const out: string[] = []
      for (const tool of enabledToolInfos) {
        if (!wanted.has(tool.key)) continue
        if (seen.has(tool.project_skills_dir)) continue
        seen.add(tool.project_skills_dir)
        out.push(tool.key)
      }
      return out
    },
    [enabledToolInfos],
  )

  const installedToolIds = useMemo(
    () => toolStatus?.installed ?? [],
    [toolStatus],
  )
  const isInstalled = useCallback(
    (id: string) => installedToolIds.includes(id),
    [installedToolIds],
  )
  const installedTools = useMemo(
    () => tools.filter((tool) => installedToolIds.includes(tool.id)),
    [tools, installedToolIds],
  )
  const toolSupportsProjectScope = useCallback(
    (toolId: string) =>
      tools.find((tool) => tool.id === toolId)?.supports_project_scope ?? true,
    [tools],
  )
  const installedProjectToolIds = useMemo(
    () => installedToolIds.filter((toolId) => toolSupportsProjectScope(toolId)),
    [installedToolIds, toolSupportsProjectScope],
  )

  const {
    setSkillScopeState,
    setScopeModalSkill,
    recentProjects,
    setRecentProjects,
    getSkillProjects,
    getSkillScope,
    handleOpenScope,
    handleCloseScope,
    handlePickProject,
    handleScopeChange,
    currentScopeModalSkill,
  } = useSkillScope({
    managedSkills,
    installedToolIds,
    installedProjectToolIds,
    loading,
    setLoading,
    setLoadingStartAt,
    setError,
    loadManagedSkills,
  })

  const staleMirrorNames = useMemo(
    () => new Set(staleMirrors.map((mirror) => mirror.name)),
    [staleMirrors],
  )

  // 点健康栏的数字时，列表只显示有问题的技能；再点一次恢复
  const [onlyProblemFilter, setOnlyProblemFilter] = useState<Set<string> | null>(null)
  const toggleProblemFilter = useCallback(() => {
    setOnlyProblemFilter((prev) => (prev ? null : new Set(problemSkillNamesRef.current)))
  }, [])
  const problemSkillNamesRef = useRef<Set<string>>(new Set())

  // 有问题的技能：断链 / 副本落后或手改 / 母库落后 —— 排序时统一置顶
  const problemSkillNames = useMemo(() => {
    const names = new Set<string>()
    for (const skill of libraryHealth?.skills ?? []) {
      if (skill.broken && skill.broken.length > 0) names.add(skill.name)
    }
    for (const key of staleBySkillTool.keys()) {
      const name = key.split('/')[0]
      if (name) names.add(name)
    }
    for (const mirror of staleMirrors) {
      names.add(mirror.name)
    }
    return names
  }, [libraryHealth, staleBySkillTool, staleMirrors])

  useEffect(() => {
    problemSkillNamesRef.current = problemSkillNames
  }, [problemSkillNames])

  const {
    searchQuery,
    setSearchQuery,
    sortBy,
    setSortBy,
    scopeFilter,
    setScopeFilter,
    toolFilter,
    setToolFilter,
    selectedTagIds,
    setSelectedTagIds,
    includeUntagged,
    visibleSkills,
    untaggedCount,
    handleToggleTagFilter,
    handleToggleUntaggedFilter,
    handleClearTagFilters,
    filterByTagId,
    filterByUntagged,
  } = useSkillFilters({
    managedSkills,
    getSkillScope,
    problemSkillNames,
    onlyProblemFilter,
  })

  const {
    tags,
    loadTags,
    tagEditorSkill,
    pendingRenameTag,
    setPendingRenameTag,
    pendingDeleteTag,
    handleCreateTag,
    handleRenameTag,
    handleCloseRenameTag,
    handleDeleteTag,
    handleCloseDeleteTag,
    handleConfirmDeleteTag,
    handleOpenEditTags,
    handleCloseEditTags,
    handleSaveSkillTags,
  } = useTagOperations({
    loading,
    setLoading,
    setLoadingStartAt,
    setActionMessage,
    setSuccessToastMessage,
    setError,
    loadManagedSkills,
    onTagDeleted: (deletedTagId) => {
      setSelectedTagIds((current) => current.filter((id) => id !== deletedTagId))
    },
    onTagRenamed: (oldTagId, newTagId) => {
      setSelectedTagIds((current) =>
        current.includes(oldTagId)
          ? current.map((id) => (id === oldTagId ? newTagId : id))
          : current,
      )
    },
  })

  useEffect(() => {
    if (isTauri) {
      loadManagedSkills()
      loadTags()
    }
  }, [isTauri, loadManagedSkills, loadTags])

  const {
    installScope,
    installProjects,
    syncTargets,
    setSyncTargets,
    pendingSyncTargetChange,
    pendingSyncTargetLabels,
    resetInstallScope,
    syncInstalledSkill,
    handleSyncTargetChange,
    handleSyncTargetChangeCancel,
    handleSyncTargetChangeConfirm,
    handleInstallScopeChange,
    handleInstallProjectsChange,
  } = useInstallScopeTargets({
    loading,
    tools,
    installedTools,
    installedToolIds,
    toolLabelById,
    sharedToolIdsByToolId,
    sharedProjectToolIdsByToolId,
    toolSupportsProjectScope,
    uniqueToolIdsBySkillsDir,
    uniqueToolIdsByProjectSkillsDir,
    isInstalled,
    defaultSyncMode,
    setActionMessage,
    setError,
    setRecentProjects,
  })

  const {
    plan,
    selected,
    variantChoice,
    showImportModal,
    reloadPlanQuietly,
    handleToggleGroup,
    handleSelectVariant,
    handleIgnoreGroup,
    handleReviewImport,
    handleCloseImport,
    handleImport,
  } = useImportPlan({
    loading,
    setLoading,
    setLoadingStartAt,
    setError,
    setActionMessage,
    setSuccessToastMessage,
    showActionErrors,
    loadManagedSkills,
    tools,
    syncTargets,
    sharedToolIdsByToolId,
    uniqueToolIdsBySkillsDir,
    isInstalled,
    defaultSyncMode,
  })

  const {
    discoveryScanSettings,
    showDiscoveryScanModal,
    discoveryScanSaving,
    discoveryScanEnabledCount,
    discoveryScanSourceCount,
    handleOpenDiscoveryScanSettings,
    handleCloseDiscoveryScanSettings,
    handleSaveDiscoveryScanSettings,
  } = useDiscoveryScan({
    onError: setError,
    onPlanReload: reloadPlanQuietly,
  })


  const {
    showAddModal,
    setShowAddModal,
    addModalTab,
    setAddModalTab,
    addModalTagIds,
    setAddModalTagIds,
    localPath,
    setLocalPath,
    gitUrl,
    setGitUrl,
    showGitPickModal,
    gitCandidates,
    gitCandidateSelected,
    showLocalPickModal,
    localCandidates,
    localCandidateSelected,
    setAutoSelectSkillName,
    handleCloseAdd,
    handleCloseGitPick,
    handleCancelGitPick,
    handleCloseLocalPick,
    handleCancelLocalPick,
    handleToggleLocalCandidate,
    handleToggleGitCandidate,
    handleCreateLocal,
    handleCreateGit,
    handleInstallSelectedLocalCandidates,
    handleInstallSelectedCandidates,
  } = useSkillImport({
    isSkillNameTaken,
    syncInstalledSkill,
    resetInstallScope,
    loadManagedSkills,
    loadTags,
    loading,
    setLoading,
    setLoadingStartAt,
    setActionMessage,
    setSuccessToastMessage,
    setError,
    showActionErrors,
  })

  const {
    pendingDeleteId,
    pendingDeleteSkill,
    handleDeletePrompt,
    handleCloseDelete,
    pendingIgnoreId,
    pendingIgnoreSkill,
    handleIgnorePrompt,
    handleCloseIgnore,
    handleDeleteManaged,
    handleIgnoreManaged,
    handleToggleSkillEnabled,
    handleUpdateSkill,
  } = useSkillLifecycle({
    managedSkills,
    loading,
    setLoading,
    setLoadingStartAt,
    setError,
    setActionMessage,
    setSuccessToastMessage,
    setSkillScopeState,
    loadManagedSkills,
    loadTags,
  })

  // 启动时的落后副本检查：后端扫描 + 自动刷新 Behind 条目，结果用 toast 汇报。
  const handleRefreshStaleOne = useCallback(
    async (skillId: string, toolId: string) => {
      try {
        await refreshStaleOne(skillId, toolId)
        toast.success(t('stale.refreshedOne'), { duration: 3200 })
        await loadManagedSkills()
        void refreshLibraryHealth()
      } catch (err) {
        const message = String(err)
        if (message.includes('STALE_STATE_CHANGED')) {
          toast.info(t('stale.stateChanged'), { duration: 4000 })
          void refreshStaleStatus()
        } else {
          toast.error(message, { duration: 4000 })
        }
      }
    },
    [
      refreshStaleOne,
      loadManagedSkills,
      refreshLibraryHealth,
      refreshStaleStatus,
      t,
    ],
  )

  useEffect(() => {
    if (!isTauri) return
    let cancelled = false
    void (async () => {
      const result = await runStartupRefresh()
      if (cancelled || !result) return
      if (result.refreshed > 0) {
        toast.success(
          t('stale.startupRefreshed', { count: result.refreshed }),
          { duration: 4000 },
        )
        await loadManagedSkills()
        void refreshLibraryHealth()
      } else if (result.remaining === 0 && result.stale_mirrors === 0) {
        // 副本与母库都一致时才说「全部同步」
        toast.success(t('stale.startupAllGood'), { duration: 3000 })
      }
      if (result.stale_mirrors > 0) {
        toast.warning(
          t('stale.mirrorsStartup', { count: result.stale_mirrors }),
          { description: t('stale.mirrorsHint'), duration: 6000 },
        )
      }
      if (result.remaining > 0) {
        toast.warning(t('stale.startupRemaining', { count: result.remaining }), {
          description: t('stale.startupRemainingHint'),
          duration: 6000,
        })
      }
    })()
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isTauri])

  const toolOptionsWithCounts = useMemo(() => {
    return installedTools.map((tool) => {
      const count = managedSkills.filter(
        (skill) => getToolSyncState(skill, tool.id, getSkillScope(skill)) === 'synced',
      ).length
      return {
        id: tool.id,
        label: tool.label,
        avatar: tool.avatar,
        count,
      }
    })
  }, [installedTools, managedSkills, getSkillScope])

  const toolCountsByToolId = useMemo(() => {
    const map: Record<string, number> = {}
    for (const opt of toolOptionsWithCounts) {
      map[opt.id] = opt.count
    }
    return map
  }, [toolOptionsWithCounts])

  const {
    bulkMode,
    bulkSelectedIds,
    bulkSelectedSkills,
    bulkSelectedNames,
    bulkHasDisabledSelected,
    bulkUpdateableSkills,
    bulkShouldEnable,
    allVisibleBulkSelected,
    showBulkSyncModal,
    showBulkDeleteModal,
    showBulkIgnoreModal,
    showBulkTagsModal,
    bulkSyncToolIds,
    handleToggleBulkMode,
    handleToggleBulkSelection,
    handleSelectVisibleSkills,
    handleOpenBulkSync,
    handleToggleBulkSyncTool,
    handleCloseBulkSync,
    handleConfirmBulkSync,
    handleOpenBulkDelete,
    handleCloseBulkDelete,
    handleConfirmBulkDelete,
    handleOpenBulkIgnore,
    handleCloseBulkIgnore,
    handleConfirmBulkIgnore,
    handleOpenBulkTags,
    handleCloseBulkTags,
    handleConfirmBulkTags,
    handleBulkUpdate,
    handleToggleBulkEnabled,
    resetBulkState,
  } = useBulkOperations({
    managedSkills,
    visibleSkills,
    installedTools,
    installedToolIds,
    installedProjectToolIds,
    toolLabelById,
    sharedToolIdsByToolId,
    sharedProjectToolIdsByToolId,
    toolSupportsProjectScope,
    getSkillScope,
    getSkillProjects,
    loading,
    setLoading,
    setLoadingStartAt,
    setActionMessage,
    setSuccessToastMessage,
    setError,
    showActionErrors,
    loadManagedSkills,
    loadTags,
  })

  const {
    pendingSharedToggle,
    pendingToolToggle,
    setPendingToolToggle,
    pendingSharedLabels,
    handleToggleToolForSkill,
    handleSyncToolWithMode,
    handleUnsyncToolForSkill,
    handlePromoteToolToOwner,
    handleConfirmToolToggle,
    handleSharedCancel,
    handleSharedConfirm,
  } = useToolSync({
    loading,
    setLoading,
    setLoadingStartAt,
    setError,
    setActionMessage,
    setSuccessToastMessage,
    loadManagedSkills,
    tools,
    getSkillScope,
    getSkillProjects,
    toolSupportsProjectScope,
    setScopeModalSkill,
    sharedToolIdsByToolId,
    toolLabelById,
  })

  const {
    storagePath,
    gitCacheTtlSecs,
    githubProxyConfig,
    toolConfig,
    handleGitCacheTtlSecsChange,
    handleGithubProxyConfigChange,
    handleToolConfigChange,
    handleClearGitCacheNow,
  } = useSystemSettings({
    setError,
    setSuccessToastMessage,
    setToolStatus,
    setSyncTargets,
  })

  const handlePickLocalPath = useCallback(async () => {
    try {
      if (!isTauri) {
        throw new Error(t('errors.notTauri'))
      }
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('selectLocalFolder'),
      })
      if (!selected || Array.isArray(selected)) return
      setLocalPath(selected)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [isTauri, t])

  const newlyInstalledToolsText = useMemo(() => {
    if (!toolStatus || toolStatus.newly_installed.length === 0) return ''
    return toolStatus.newly_installed
      .map((id) => tools.find((t) => t.id === id)?.label ?? id)
      .join('、')
  }, [toolStatus, tools])

  const handleOpenSettings = useCallback(() => {
    setShowAddModal(false)
    setActiveView('settings')
  }, [])




  const handleViewChange = useCallback(
    (view: 'myskills' | 'explore' | 'manage' | 'library') => {
      setShowAddModal(false)
      setActiveView(view)
      if (view !== 'myskills') {
        resetBulkState()
      }
      if (view === 'explore') {
        loadFeaturedSkills()
      }
      if (view === 'manage') {
        setManagementTab('tags')
      }
      if (view === 'myskills') {
        setDetailSkill(null)
      }
    },
    [loadFeaturedSkills, resetBulkState],
  )

  const handleOpenDetail = useCallback((skill: ManagedSkill) => {
    resetBulkState()
    setDetailSkill(skill)
    setActiveView('detail')
  }, [resetBulkState])

  const handleBackToList = useCallback(() => {
    setDetailSkill(null)
    setActiveView('myskills')
  }, [])



  const handleOpenAdd = useCallback((tab: 'git' | 'local' = 'git') => {
    resetInstallScope()
    setAddModalTab(tab)
    setShowAddModal(true)
    setAddModalTagIds([])
  }, [resetInstallScope])

  const handleCancelLoading = useCallback(() => {
    void invokeTauri('cancel_current_operation').catch(() => {})
    setLoading(false)
    setLoadingStartAt(null)
    setActionMessage(null)
  }, [invokeTauri])



  const handleCloseSettings = useCallback(() => {
    setActiveView('myskills')
  }, [])

  const handleThemeChange = useCallback(
    (nextTheme: 'system' | 'light' | 'dark') => {
      setThemePreference(nextTheme)
    },
    [],
  )

  const handleCloseNewTools = useCallback(() => {
    if (!loading) setShowNewToolsModal(false)
  }, [loading])

  const handleSortChange = useCallback((value: 'updated' | 'name') => {
    setSortBy(value)
  }, [])

  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value)
  }, [])

  const handleScopeFilterChange = useCallback(
    (value: 'all' | 'global' | 'project') => {
      setScopeFilter(value)
    },
    [],
  )

  const handleToggleAddModalTag = useCallback((tagId: number) => {
    setAddModalTagIds((current) =>
      current.includes(tagId)
        ? current.filter((id) => id !== tagId)
        : [...current, tagId],
    )
  }, [])

  const handleReviewUntagged = useCallback(() => {
    filterByUntagged()
    setActiveView('myskills')
  }, [filterByUntagged])

  const handleViewTag = useCallback((tagId: number) => {
    filterByTagId(tagId)
    setActiveView('myskills')
  }, [filterByTagId])

  const handleOpenTagsPage = useCallback(() => {
    setManagementTab('tags')
    setActiveView('manage')
  }, [])









  // 工具属性（如「仅复制」）改动后，重新拉取一次状态，让列表、右键菜单和
  // 导入流程立刻看到新能力，而不用等下次启动。
  const reloadToolStatus = useCallback(async () => {
    if (!isTauri) return
    try {
      const status = await invokeTauri<ToolStatusDto>('get_tool_status')
      setToolStatus(status)
    } catch (err) {
      console.warn(err)
    }
  }, [])

  useEffect(() => {
    const load = async () => {
      if (!isTauri) return
      try {
        const status = await invokeTauri<ToolStatusDto>('get_tool_status')
        setToolStatus(status)

        // Default-select installed tools for sync targets if user hasn't toggled yet.
        setSyncTargets((prev) => {
          if (Object.keys(prev).length > 0) return prev
          const next: Record<string, boolean> = {}
          for (const t of status.tools) {
            next[t.key] = status.installed.includes(t.key)
          }
          return next
        })

        if (status.newly_installed.length > 0) {
          setShowNewToolsModal(true)
        }
      } catch (err) {
        // Non-fatal; app can still work without detection.
        console.warn(err)
      }
    }
    void load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isTauri])



  const [exploreInstallTrigger, setExploreInstallTrigger] = useState(0)
  const exploreInstallUrlRef = useRef<string | null>(null)

  const handleExploreInstall = useCallback(
    (sourceUrl: string, skillName?: string) => {
      resetInstallScope()
      setGitUrl(sourceUrl)
      if (skillName) setAutoSelectSkillName(skillName)
      if (toolStatus) {
        const targets: Record<string, boolean> = {}
        for (const id of toolStatus.installed) {
          targets[id] = true
        }
        setSyncTargets(targets)
      }
      exploreInstallUrlRef.current = sourceUrl
      setExploreInstallTrigger((n) => n + 1)
    },
    [resetInstallScope, toolStatus],
  )

  useEffect(() => {
    if (exploreInstallTrigger > 0 && exploreInstallUrlRef.current && !loading) {
      exploreInstallUrlRef.current = null
      void handleCreateGit()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [exploreInstallTrigger])


  const handleSyncAllNewTools = useCallback(() => {
    if (!toolStatus) return
    // Only register the new tools as sync targets (tick their checkboxes).
    // Do NOT push all managed skills to them — the user can sync per-skill.
    setSyncTargets((prev) => {
      const next = { ...prev }
      for (const id of toolStatus.newly_installed) {
        const shared = sharedToolIdsByToolId[id] ?? [id]
        for (const sid of shared) next[sid] = true
      }
      return next
    })
    setShowNewToolsModal(false)
    reloadPlanQuietly()
  }, [reloadPlanQuietly, setSyncTargets, sharedToolIdsByToolId, toolStatus])









  const globalSkillCount = managedSkills.filter(
    (skill) => getSkillScope(skill) === 'global',
  ).length
  const projectSkillCount = managedSkills.length - globalSkillCount
  const enabledSkillCount = managedSkills.filter((skill) => skill.enabled !== false).length
  const syncIssueCount = managedSkills.filter((skill) => {
    const state = getSkillSyncState(skill)
    return state === 'source-error' || state === 'partial' || state === 'failed'
  }).length
  const unsyncedSkillCount = managedSkills.filter(
    (skill) => getSkillSyncState(skill) === 'idle',
  ).length
  const dashboardSyncStatus =
    syncIssueCount > 0
      ? { className: 'error', label: t('stats.needsAttention', { count: syncIssueCount }) }
      : unsyncedSkillCount > 0
        ? { className: 'idle', label: t('stats.notSyncedCount', { count: unsyncedSkillCount }) }
        : enabledSkillCount === managedSkills.length
          ? { className: 'healthy', label: t('stats.allNormal') }
          : { className: 'idle', label: t('stats.enabledCount', { count: enabledSkillCount }) }

  const handleManagementTabChange = (tab: ManagementTab) => {
    setShowAddModal(false)
    setManagementTab(tab)
    setActiveView('manage')
  }

  return (
    <div className={`skills-app${isTauri ? ' is-tauri' : ''}${sidebarCollapsed ? ' sidebar-collapsed' : ''}`}>
      <Toaster
        position="top-right"
        richColors
        toastOptions={{ duration: 1800 }}
      />
      <LoadingOverlay
        loading={loading}
        actionMessage={actionMessage}
        loadingStartAt={loadingStartAt}
        onCancel={handleCancelLoading}
        t={t}
      />
      <WindowResizeHandles enabled={isTauri} />

      <TitleBar t={t} />

      <div className="skills-body">
        <Sidebar
          activeView={activeView}
          managementTab={managementTab}
          skillCount={managedSkills.length}
          tagCount={tags.length}
          toolCount={toolStatus?.tools.length ?? 0}
          collapsed={sidebarCollapsed}
          onToggleCollapsed={() => setSidebarCollapsed((collapsed) => !collapsed)}
          onOpenSettings={handleOpenSettings}
          onViewChange={handleViewChange}
          onManagementTabChange={handleManagementTabChange}
          t={t}
        />

        <main className="skills-main">
        {activeView === 'library' ? (
          <CentralLibraryView onOpenFolder={(path) => { void openPath(path) }} />
        ) : activeView === 'detail' && detailSkill ? (
          <SkillDetailView
            skill={detailSkill}
            onBack={handleBackToList}
            invokeTauri={invokeTauri}
            formatRelative={formatRelative}
            tools={installedTools}
            scope={getSkillScope(detailSkill)}
            projects={getSkillProjects(detailSkill)}
            t={t}
            onRefresh={loadManagedSkills}
          />
        ) : activeView === 'myskills' ? (
          <div className="dashboard-stack">
            <section className="dashboard-stats" aria-label={t('navMySkills')}>
              <article>
                <span>{t('stats.managed')}</span>
                <strong>{managedSkills.length}</strong>
              </article>
              <article>
                <span>{t('stats.global')}</span>
                <strong>{globalSkillCount}</strong>
              </article>
              <article>
                <span>{t('stats.project')}</span>
                <strong>{projectSkillCount}</strong>
              </article>
              <article className={`dashboard-status-card${syncIssueCount > 0 ? ' actionable' : ''}`}>
                <button
                  className="dashboard-status-button"
                  type="button"
                  disabled={syncIssueCount === 0}
                  onClick={() => handleViewChange('myskills')}
                  aria-label={
                    syncIssueCount > 0
                      ? `${dashboardSyncStatus.label}，${t('stats.viewIssues')}`
                      : dashboardSyncStatus.label
                  }
                  title={syncIssueCount > 0 ? t('stats.viewIssues') : undefined}
                >
                  <span>{t('stats.syncStatus')}</span>
                  <strong className={`status-summary ${dashboardSyncStatus.className}`}>
                    <i />{dashboardSyncStatus.label}
                  </strong>
                  {syncIssueCount > 0 ? (
                    <span className="dashboard-status-action">
                      {t('stats.viewIssues')} <span aria-hidden="true">→</span>
                    </span>
                  ) : null}
                </button>
              </article>
            </section>
            {libraryHealth ? (
              <div className="library-health-row">
              <button
                type="button"
                className={`library-health-bar${libraryHealth.summary.broken_targets > 0 ? ' has-broken' : ''}`}
                onClick={handleHealthBarClick}
                aria-label={t('health.title')}
                title={
                  libraryHealth.summary.broken_targets > 0
                    ? t('health.brokenDetailsTitle', {
                        count: libraryHealth.summary.broken_targets,
                      })
                    : t('health.refreshTitle')
                }
              >
                <span className="library-health-item">
                  {t('health.total', { count: libraryHealth.summary.total })}
                </span>
                <span className="library-health-sep" aria-hidden="true">·</span>
                <span className="library-health-item is-ok">
                  {t('health.synced', { count: libraryHealth.summary.synced_skills })}
                </span>
                <span className="library-health-sep" aria-hidden="true">·</span>
                <span
                  className={`library-health-item${libraryHealth.summary.broken_targets > 0 ? ' is-error' : ''}`}
                >
                  {t('health.broken', { count: libraryHealth.summary.broken_targets })}
                </span>
                <span className="library-health-sep" aria-hidden="true">·</span>
                <span
                  className={`library-health-item${staleCount > 0 ? ' is-stale' : ''}`}
                  role="button"
                  tabIndex={0}
                  title={staleCount > 0 ? t('stale.detailsTitle', { count: staleCount }) : undefined}
                  onClick={(e) => {
                    e.stopPropagation()
                    if (staleCount > 0) {
                      toggleProblemFilter()
                    } else {
                      handleStaleCountClick()
                    }
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.stopPropagation()
                      handleStaleCountClick()
                    }
                  }}
                >
                  {t('health.stale', { count: staleCount })}
                </span>
                <span className="library-health-sep" aria-hidden="true">·</span>
                <span
                  className={`library-health-item${staleMirrorCount > 0 ? ' is-stale' : ''}`}
                  role="button"
                  tabIndex={0}
                  title={
                    staleMirrorCount > 0
                      ? t('stale.mirrorsDetailsTitle', { count: staleMirrorCount })
                      : undefined
                  }
                  onClick={(e) => {
                    e.stopPropagation()
                    if (staleMirrorCount === 0) {
                      void refreshStaleMirrors()
                      return
                    }
                    toggleProblemFilter()
                    return;
                    const lines = staleMirrors
                      .slice(0, 8)
                      .map((mirror) => mirror.name)
                    toast.warning(
                      t('stale.mirrorsDetailsTitle', { count: staleMirrorCount }),
                      {
                        description: `${lines.join('、')}\n${t('stale.mirrorsHint')}`,
                        duration: 8000,
                      },
                    )
                  }}
                >
                  {t('health.mirror', { count: staleMirrorCount })}
                </span>
                <span className="library-health-sep" aria-hidden="true">·</span>
                <span className="library-health-item is-muted">
                  {t('health.idle', { count: libraryHealth.summary.idle_skills })}
                </span>
              </button>
              <button
                type="button"
                className="filter-refresh-btn"
                onClick={() => void refreshAll()}
                title="刷新列表和状态（F5）"
                aria-label="刷新"
              >
                <RefreshCw size={13} />
              </button>
              </div>
            ) : null}
            {onlyProblemFilter ? (
              <div className="problem-filter-banner">
                <span>只显示有问题的技能（{onlyProblemFilter.size} 个）</span>
                <button type="button" onClick={() => setOnlyProblemFilter(null)}>
                  显示全部
                </button>
              </div>
            ) : null}
            <FilterBar
              onRefresh={refreshAll}
              sortBy={sortBy}
              searchQuery={searchQuery}
              scopeFilter={scopeFilter}
              toolFilter={toolFilter}
              toolOptions={toolOptionsWithCounts}
              tags={tags}
              selectedTagIds={selectedTagIds}
              includeUntagged={includeUntagged}
              untaggedCount={untaggedCount}
              totalCount={visibleSkills.length}
              bulkMode={bulkMode}
              bulkSelectedCount={bulkSelectedIds.length}
              viewMode={skillViewMode}
              onSortChange={handleSortChange}
              onSearchChange={handleSearchChange}
              onScopeFilterChange={handleScopeFilterChange}
              onToolFilterChange={setToolFilter}
              onToggleTag={handleToggleTagFilter}
              onToggleUntagged={handleToggleUntaggedFilter}
              onClearTags={handleClearTagFilters}
              onManageTags={handleOpenTagsPage}
              onToggleBulkMode={handleToggleBulkMode}
              onViewModeChange={setSkillViewMode}
              t={t}
            />
            <SkillsList
              peerVersionLabels={peerVersionLabels}
              staleMirrorNames={staleMirrorNames}
              onRefreshMirror={handleRefreshMirror}
              plan={plan}
              visibleSkills={visibleSkills}
              installedTools={installedTools}
              toolCounts={toolCountsByToolId}
              loading={loading}
              bulkMode={bulkMode}
              selectedSkillIds={bulkSelectedIds}
              viewMode={skillViewMode}
              getGithubInfo={getGithubInfo}
              getSkillSourceLabel={getSkillSourceLabel}
              formatRelative={formatRelative}
              onReviewImport={handleReviewImport}
              onOpenScanSettings={handleOpenDiscoveryScanSettings}
              onUpdateSkill={handleUpdateSkill}
              onDeleteSkill={handleDeletePrompt}
              onIgnoreSkill={handleIgnorePrompt}
              onToggleSkillEnabled={handleToggleSkillEnabled}
              onToggleTool={handleToggleToolForSkill}
              onSyncToolWithMode={handleSyncToolWithMode}
              onUnsyncTool={handleUnsyncToolForSkill}
              onPromoteToolToOwner={handlePromoteToolToOwner}
              onOpenScope={handleOpenScope}
              onOpenDetail={handleOpenDetail}
              onEditTags={handleOpenEditTags}
              onToggleBulkSelection={handleToggleBulkSelection}
              getSkillScope={getSkillScope}
              getSkillProjects={getSkillProjects}
              skillHealthByName={skillHealthByName}
              sharedDirs={sharedSkillDirs}
              toolInstanceTotals={toolInstanceTotals}
              staleBySkillTool={staleBySkillTool}
              onRefreshStale={handleRefreshStaleOne}
              t={t}
            />
            {bulkMode ? (
              <div className="bulk-action-bar">
                <div className="bulk-action-copy">
                  <strong>{t('bulk.selected', { count: bulkSelectedIds.length })}</strong>
                  <span>{t('bulk.helper')}</span>
                </div>
                <div className="bulk-action-buttons">
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleSelectVisibleSkills}
                    disabled={loading || visibleSkills.length === 0}
                  >
                    {allVisibleBulkSelected
                      ? t('bulk.unselectVisible')
                      : t('bulk.selectVisible')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleOpenBulkTags}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {t('bulk.tags')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleOpenBulkSync}
                    disabled={
                      loading ||
                      bulkSelectedIds.length === 0 ||
                      bulkHasDisabledSelected
                    }
                  >
                    {t('bulk.sync')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={() => void handleBulkUpdate()}
                    disabled={loading || bulkUpdateableSkills.length === 0}
                    title={
                      bulkSelectedIds.length > 0 && bulkUpdateableSkills.length === 0
                        ? t('bulk.noUpdateSources')
                        : undefined
                    }
                  >
                    {t('bulk.update')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={() => void handleToggleBulkEnabled()}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {bulkShouldEnable ? t('bulk.enable') : t('bulk.disable')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleOpenBulkIgnore}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {t('bulk.ignore', { defaultValue: '屏蔽' })}
                  </button>
                  <button
                    className="btn btn-danger"
                    type="button"
                    onClick={handleOpenBulkDelete}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {t('bulk.delete')}
                  </button>
                </div>
              </div>
            ) : null}
          </div>
        ) : activeView === 'manage' ? (
          <div className="management-page">
            <div className="management-header">
              <div className="management-heading">
                <h1>{t('manageCenterTitle')}</h1>
                <p>{t('manageCenterHelp')}</p>
              </div>
              <div className="management-tab-list" role="tablist" aria-label={t('manageCenterTitle')}>
                <button
                  className={`management-tab${managementTab === 'tags' ? ' active' : ''}`}
                  type="button"
                  role="tab"
                  aria-selected={managementTab === 'tags'}
                  onClick={() => setManagementTab('tags')}
                >
                  {t('manageTabs.tags')}
                </button>
                <button
                  className={`management-tab${managementTab === 'tools' ? ' active' : ''}`}
                  type="button"
                  role="tab"
                  aria-selected={managementTab === 'tools'}
                  onClick={() => setManagementTab('tools')}
                >
                  {t('manageTabs.tools')}
                </button>
              </div>
            </div>
            <div className="management-panel">
              {managementTab === 'tags' ? (
                <TagsPage
                  embedded
                  tags={tags}
                  untaggedCount={untaggedCount}
                  loading={loading}
                  formatRelative={formatRelative}
                  onReviewUntagged={handleReviewUntagged}
                  onViewTag={handleViewTag}
                  onCreateTag={handleCreateTag}
                  onRenameTag={setPendingRenameTag}
                  onDeleteTag={handleDeleteTag}
                  t={t}
                />
              ) : (
                <ToolsPage
                  embedded
                  toolStatus={toolStatus}
                  toolConfig={toolConfig}
                  onToolConfigChange={handleToolConfigChange}
                  onToolStatusRefresh={reloadToolStatus}
                  t={t}
                />
              )}
            </div>
          </div>
        ) : activeView === 'settings' ? (
          <SettingsPage
            isTauri={isTauri}
            language={language}
            storagePath={storagePath}
            gitCacheTtlSecs={gitCacheTtlSecs}
            themePreference={themePreference}
            autoRefreshStaleCopies={autoRefreshEnabled}
            onToggleLanguage={toggleLanguage}
            onThemeChange={handleThemeChange}
            onAutoRefreshStaleCopiesChange={handleAutoRefreshStaleChange}
            onGitCacheTtlSecsChange={handleGitCacheTtlSecsChange}
            onClearGitCacheNow={handleClearGitCacheNow}
            githubProxyConfig={githubProxyConfig}
            onGithubProxyConfigChange={handleGithubProxyConfigChange}
            discoveryScanEnabledCount={discoveryScanEnabledCount}
            discoveryScanSourceCount={discoveryScanSourceCount}
            onOpenDiscoveryScanSettings={handleOpenDiscoveryScanSettings}
            onBack={handleCloseSettings}
            t={t}
          />
        ) : (
          <ExplorePage
            featuredSkills={featuredSkills}
            featuredLoading={featuredLoading}
            exploreFilter={exploreFilter}
            searchResults={searchResults}
            searchLoading={searchLoading}
            managedSkills={managedSkills}
            loading={loading}
            onExploreFilterChange={handleExploreFilterChange}
            onInstallSkill={handleExploreInstall}
            onOpenManualAdd={handleOpenAdd}
            t={t}
          />
        )}
      </main>
      </div>

      <AddSkillModal
        open={showAddModal}
        loading={loading}
        canClose={!loading}
        addModalTab={addModalTab}
        localPath={localPath}
        gitUrl={gitUrl}
        tags={tags}
        selectedTagIds={addModalTagIds}
        syncTargets={syncTargets}
        installedTools={installedTools}
        toolStatus={toolStatus}
        installScope={installScope}
        installProjects={installProjects}
        recentProjects={recentProjects}
        onRequestClose={handleCloseAdd}
        onTabChange={setAddModalTab}
        onLocalPathChange={setLocalPath}
        onPickLocalPath={handlePickLocalPath}
        onGitUrlChange={setGitUrl}
        onToggleTag={handleToggleAddModalTag}
        onSyncTargetChange={handleSyncTargetChange}
        onInstallScopeChange={handleInstallScopeChange}
        onInstallProjectsChange={handleInstallProjectsChange}
        onPickProject={handlePickProject}
        onSubmit={addModalTab === 'local' ? handleCreateLocal : handleCreateGit}
        t={t}
      />

      <EditSkillTagsModal
        key={
          tagEditorSkill
            ? `${tagEditorSkill.id}-${tagEditorSkill.tags.map((tag) => tag.id).join('-')}`
            : 'edit-tags'
        }
        open={Boolean(tagEditorSkill)}
        loading={loading}
        skill={
          tagEditorSkill
            ? managedSkills.find((skill) => skill.id === tagEditorSkill.id) ?? tagEditorSkill
            : null
        }
        tags={tags}
        onRequestClose={handleCloseEditTags}
        onSave={handleSaveSkillTags}
        t={t}
      />

      <BulkSyncModal
        open={showBulkSyncModal}
        loading={loading}
        selectedCount={bulkSelectedIds.length}
        installedTools={installedTools}
        selectedToolIds={bulkSyncToolIds}
        onToggleTool={handleToggleBulkSyncTool}
        onRequestClose={handleCloseBulkSync}
        onConfirm={handleConfirmBulkSync}
        t={t}
      />

      <BulkTagsModal
        open={showBulkTagsModal}
        loading={loading}
        selectedSkills={bulkSelectedSkills}
        tags={tags}
        onRequestClose={handleCloseBulkTags}
        onConfirm={handleConfirmBulkTags}
        t={t}
      />

      <BulkDeleteModal
        open={showBulkDeleteModal}
        loading={loading}
        skillNames={bulkSelectedNames}
        onRequestClose={handleCloseBulkDelete}
        onConfirm={handleConfirmBulkDelete}
        onIgnore={() => {
          handleCloseBulkDelete()
          handleOpenBulkIgnore()
        }}
        t={t}
      />

      <BulkIgnoreModal
        open={showBulkIgnoreModal}
        loading={loading}
        skillNames={bulkSelectedNames}
        onRequestClose={handleCloseBulkIgnore}
        onConfirm={() => void handleConfirmBulkIgnore()}
        t={t}
      />

      {showImportModal && plan ? (
        <ImportModal
          open={showImportModal}
          loading={loading}
          plan={plan}
          selected={selected}
          variantChoice={variantChoice}
          onRequestClose={handleCloseImport}
          onToggleGroup={handleToggleGroup}
          onSelectVariant={handleSelectVariant}
          onIgnoreGroup={handleIgnoreGroup}
          onImport={handleImport}
          t={t}
        />
      ) : null}

      <DiscoveryScanModal
        open={showDiscoveryScanModal}
        saving={discoveryScanSaving}
        settings={discoveryScanSettings}
        onRequestClose={handleCloseDiscoveryScanSettings}
        onSave={handleSaveDiscoveryScanSettings}
        t={t}
      />

      <ToggleToolConfirmModal
        open={Boolean(pendingToolToggle)}
        loading={loading}
        skillName={pendingToolToggle?.skill.name ?? ''}
        toolLabel={
          pendingToolToggle
            ? (toolLabelById[pendingToolToggle.toolId] ??
              t(`tools.${pendingToolToggle.toolId}`, {
                defaultValue: pendingToolToggle.toolId,
              }))
            : ''
        }
        action={pendingToolToggle?.action ?? 'sync'}
        onRequestClose={() => setPendingToolToggle(null)}
        onConfirm={handleConfirmToolToggle}
        t={t}
      />

      <SharedDirModal
        open={Boolean(pendingSharedToggle)}
        loading={loading}
        toolLabel={pendingSharedLabels?.toolLabel ?? ''}
        otherLabels={pendingSharedLabels?.otherLabels ?? ''}
        onRequestClose={handleSharedCancel}
        onConfirm={handleSharedConfirm}
        t={t}
      />

      <SharedDirModal
        open={Boolean(pendingSyncTargetChange)}
        loading={loading}
        toolLabel={pendingSyncTargetLabels?.toolLabel ?? ''}
        otherLabels={pendingSyncTargetLabels?.otherLabels ?? ''}
        onRequestClose={handleSyncTargetChangeCancel}
        onConfirm={handleSyncTargetChangeConfirm}
        t={t}
      />

      <RenameTagModal
        key={pendingRenameTag?.id ?? 'rename-tag-modal'}
        open={Boolean(pendingRenameTag)}
        loading={loading}
        tag={pendingRenameTag}
        onRequestClose={handleCloseRenameTag}
        onSave={(tagId, name) => void handleRenameTag(tagId, name)}
        t={t}
      />

      <ScopeSyncModal
        key={
          currentScopeModalSkill
            ? `${currentScopeModalSkill.id}-${getSkillScope(currentScopeModalSkill)}`
            : 'scope-modal'
        }
        open={Boolean(currentScopeModalSkill)}
        loading={loading}
        skill={currentScopeModalSkill}
        scope={
          currentScopeModalSkill ? getSkillScope(currentScopeModalSkill) : 'global'
        }
        projects={
          currentScopeModalSkill ? getSkillProjects(currentScopeModalSkill) : []
        }
        recentProjects={recentProjects}
        onRequestClose={handleCloseScope}
        onScopeChange={handleScopeChange}
        onPickProject={handlePickProject}
        t={t}
      />

      <NewToolsModal
        open={Boolean(showNewToolsModal && newlyInstalledToolsText)}
        loading={loading}
        toolsLabelText={newlyInstalledToolsText}
        onLater={handleCloseNewTools}
        onSyncAll={handleSyncAllNewTools}
        t={t}
      />

      <DeleteModal
        open={Boolean(pendingDeleteId)}
        loading={loading}
        skillName={pendingDeleteSkill?.name ?? null}
        onRequestClose={handleCloseDelete}
        onConfirm={() => {
          if (pendingDeleteSkill) void handleDeleteManaged(pendingDeleteSkill)
        }}
        onIgnore={() => {
          if (pendingDeleteSkill) void handleIgnoreManaged(pendingDeleteSkill)
        }}
        t={t}
      />

      <IgnoreModal
        open={Boolean(pendingIgnoreId)}
        loading={loading}
        skillName={pendingIgnoreSkill?.name ?? null}
        onRequestClose={handleCloseIgnore}
        onConfirm={() => {
          if (pendingIgnoreSkill) void handleIgnoreManaged(pendingIgnoreSkill)
        }}
        t={t}
      />

      {pendingDeleteTag ? (
        <div className="modal-backdrop" onClick={loading ? undefined : handleCloseDeleteTag}>
          <div
            className="modal modal-delete tag-delete-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <div className="modal-title">{t('deleteTagTitle')}</div>
              <button
                className="modal-close"
                type="button"
                onClick={handleCloseDeleteTag}
                disabled={loading}
              >
                ×
              </button>
            </div>
            <div className="modal-body tag-delete-body">
              {t('deleteTagConfirm', {
                name: pendingDeleteTag.name,
                count: pendingDeleteTag.skill_count,
              })}
            </div>
            <div className="modal-footer">
              <button
                className="btn btn-secondary"
                type="button"
                onClick={handleCloseDeleteTag}
                disabled={loading}
              >
                {t('cancel')}
              </button>
              <button
                className="btn btn-danger"
                type="button"
                onClick={() => void handleConfirmDeleteTag()}
                disabled={loading}
              >
                {t('deleteAction')}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {showLocalPickModal ? (
        <LocalPickModal
          open={showLocalPickModal}
          loading={loading}
          localCandidates={localCandidates}
          localCandidateSelected={localCandidateSelected}
          onRequestClose={handleCloseLocalPick}
          onCancel={handleCancelLocalPick}
          onToggleCandidate={handleToggleLocalCandidate}
          onInstall={handleInstallSelectedLocalCandidates}
          t={t}
        />
      ) : null}

      {showGitPickModal ? (
        <GitPickModal
          open={showGitPickModal}
          loading={loading}
          gitCandidates={gitCandidates}
          gitCandidateSelected={gitCandidateSelected}
          onRequestClose={handleCloseGitPick}
          onCancel={handleCancelGitPick}
          onToggleCandidate={handleToggleGitCandidate}
          onInstall={handleInstallSelectedCandidates}
          t={t}
        />
      ) : null}

      </div>
  )
}

export default App
