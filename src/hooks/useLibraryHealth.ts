import { useCallback, useEffect, useState } from 'react'
import { isTauri, invokeTauri } from './useTauriInvoke'
import type { LibraryHealthReportDto } from '../components/skills/types'

/**
 * Loads the backend library health report (sync targets liveness + idle
 * skills). Health is an auxiliary signal: failures are logged instead of
 * surfacing a blocking error toast.
 */
export function useLibraryHealth() {
  const [libraryHealth, setLibraryHealth] = useState<LibraryHealthReportDto | null>(
    null,
  )
  const [libraryHealthLoading, setLibraryHealthLoading] = useState(false)

  const refreshLibraryHealth = useCallback(async () => {
    if (!isTauri) return
    setLibraryHealthLoading(true)
    try {
      const report =
        await invokeTauri<LibraryHealthReportDto>('get_library_health')
      setLibraryHealth(report)
    } catch (err) {
      // Non-fatal; the app still works without health badges.
      console.warn(err)
    } finally {
      setLibraryHealthLoading(false)
    }
  }, [])

  useEffect(() => {
    void refreshLibraryHealth()
  }, [refreshLibraryHealth])

  return {
    libraryHealth,
    libraryHealthLoading,
    refreshLibraryHealth,
  }
}
