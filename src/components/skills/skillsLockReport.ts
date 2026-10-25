import type { LockDiffReportDto } from './types'

export type LockDiffSectionKey = 'missing' | 'extra' | 'drifted' | 'targets'

export type LockDiffSection = {
  key: LockDiffSectionKey
  /** Total entries in this category (used for the heading count). */
  total: number
  /** First `limit` entry names rendered in the report. */
  entries: string[]
  /** How many entries exist beyond the rendered preview. */
  hiddenCount: number
}

export const LOCK_DIFF_PREVIEW_LIMIT = 5

/**
 * Splits a backend lock diff report into display sections. Only the first
 * `limit` names per section are rendered; the rest are summarised as a
 * "+N more" count so huge manifests stay readable.
 */
export function buildLockDiffSections(
  report: LockDiffReportDto,
  limit: number = LOCK_DIFF_PREVIEW_LIMIT,
): LockDiffSection[] {
  const groups: Array<{ key: LockDiffSectionKey; entries: string[] }> = [
    { key: 'missing', entries: report.missing_here.map((skill) => skill.name) },
    { key: 'extra', entries: [...report.extra_here] },
    { key: 'drifted', entries: report.drifted.map((drift) => drift.name) },
    { key: 'targets', entries: [...report.targets_changed] },
  ]

  return groups.map((group) => ({
    key: group.key,
    total: group.entries.length,
    entries: group.entries.slice(0, Math.max(0, limit)),
    hiddenCount: Math.max(0, group.entries.length - limit),
  }))
}

/** True when the library matches the manifest exactly. */
export function hasLockDifferences(report: LockDiffReportDto): boolean {
  return (
    report.missing_here.length > 0 ||
    report.extra_here.length > 0 ||
    report.drifted.length > 0 ||
    report.targets_changed.length > 0
  )
}
