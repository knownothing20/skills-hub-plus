import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { isTauri, invokeTauri } from './useTauriInvoke'
import type {
  StartupStaleRefreshDto,
  StaleCopyStatusDto,
} from '../components/skills/types'

/**
 * 落后副本状态：加载扫描结果、启动时自动刷新、手动刷新单个目标。
 *
 * 判定与刷新都在后端（core::stale_copies），这里只做数据搬运：
 * - `staleBySkillTool` 以 `skillName/tool` 为键，供卡片角标直接查
 * - 启动刷新只跑一次（`startupDoneRef` 防抖），结果交给调用方 toast
 */
export function useStaleCopies() {
  const [staleStatuses, setStaleStatuses] = useState<StaleCopyStatusDto[]>([])
  const [autoRefreshEnabled, setAutoRefreshEnabled] = useState(true)
  const startupDoneRef = useRef(false)

  const refreshStaleStatus = useCallback(async () => {
    if (!isTauri) return
    try {
      const statuses =
        await invokeTauri<StaleCopyStatusDto[]>('get_stale_copy_status')
      setStaleStatuses(statuses)
    } catch (err) {
      // Non-fatal: badges are an auxiliary signal.
      console.warn(err)
    }
  }, [])

  const setAutoRefresh = useCallback(async (enabled: boolean) => {
    const next = await invokeTauri<boolean>('set_auto_refresh_stale_copies_cmd', {
      enabled,
    })
    setAutoRefreshEnabled(next)
  }, [])

  /**
   * 启动时调用一次：扫描 + 自动刷新 Behind 条目。
   * 返回结果给调用方决定 toast 文案；结束后刷新列表状态。
   */
  const runStartupRefresh = useCallback(async (): Promise<StartupStaleRefreshDto | null> => {
    if (!isTauri || startupDoneRef.current) return null
    startupDoneRef.current = true
    try {
      const result = await invokeTauri<StartupStaleRefreshDto>('run_startup_stale_refresh')
      await refreshStaleStatus()
      return result
    } catch (err) {
      console.warn(err)
      return null
    }
  }, [refreshStaleStatus])

  /** 手动刷新一个落后副本（点卡片角标）。抛错交给调用方 toast。 */
  const refreshOne = useCallback(
    async (skillId: string, tool: string) => {
      await invokeTauri('refresh_stale_copy_cmd', { skillId, tool })
      await refreshStaleStatus()
    },
    [refreshStaleStatus],
  )

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const [statuses, enabled] = await Promise.all([
        invokeTauri<StaleCopyStatusDto[]>('get_stale_copy_status').catch(() => null),
        invokeTauri<boolean>('get_auto_refresh_stale_copies').catch(() => null),
      ])
      if (cancelled) return
      if (statuses) setStaleStatuses(statuses)
      if (enabled !== null && enabled !== undefined) setAutoRefreshEnabled(enabled)
    })()
    return () => {
      cancelled = true
    }
  }, [])

  const staleBySkillTool = useMemo(
    () =>
      new Map(staleStatuses.map((status) => [`${status.skill_name}/${status.tool}`, status])),
    [staleStatuses],
  )

  return {
    staleStatuses,
    staleBySkillTool,
    staleCount: staleStatuses.length,
    autoRefreshEnabled,
    setAutoRefresh,
    refreshStaleStatus,
    runStartupRefresh,
    refreshOne,
  }
}
