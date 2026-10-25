import { describe, expect, it } from 'vitest'

import type { LockDiffReportDto } from './types'
import { buildLockDiffSections, hasLockDifferences } from './skillsLockReport'

function makeReport(overrides: Partial<LockDiffReportDto> = {}): LockDiffReportDto {
  return {
    lock_path: 'D:\\locks\\skills.lock',
    missing_here: [],
    extra_here: [],
    drifted: [],
    targets_changed: [],
    ...overrides,
  }
}

describe('buildLockDiffSections', () => {
  it('returns four empty sections for a matching library', () => {
    const sections = buildLockDiffSections(makeReport())
    expect(sections.map((section) => section.key)).toEqual([
      'missing',
      'extra',
      'drifted',
      'targets',
    ])
    expect(sections.every((section) => section.total === 0 && section.entries.length === 0)).toBe(
      true,
    )
    expect(sections.every((section) => section.hiddenCount === 0)).toBe(true)
  })

  it('maps each report category to its section names', () => {
    const report = makeReport({
      missing_here: [
        {
          name: 'alpha',
          enabled: true,
          tags: [],
          source_ref: null,
          content_hash: 'h1',
          targets: [],
        },
      ],
      extra_here: ['bonus'],
      drifted: [{ name: 'edited', lock_hash: 'a', local_hash: 'b' }],
      targets_changed: ['retargeted'],
    })

    const sections = buildLockDiffSections(report)
    expect(sections.map((section) => section.entries)).toEqual([
      ['alpha'],
      ['bonus'],
      ['edited'],
      ['retargeted'],
    ])
    expect(sections.every((section) => section.total === 1)).toBe(true)
  })

  it('previews only the first entries and reports the remainder', () => {
    const report = makeReport({
      extra_here: ['a', 'b', 'c', 'd', 'e', 'f', 'g'],
    })

    const sections = buildLockDiffSections(report)
    const extra = sections.find((section) => section.key === 'extra')
    expect(extra?.entries).toEqual(['a', 'b', 'c', 'd', 'e'])
    expect(extra?.total).toBe(7)
    expect(extra?.hiddenCount).toBe(2)
  })

  it('supports a custom preview limit', () => {
    const report = makeReport({ drifted: [{ name: 'x', lock_hash: '1', local_hash: '2' }] })
    const sections = buildLockDiffSections(report, 0)
    expect(sections.every((section) => section.entries.length === 0)).toBe(true)
    expect(sections.find((section) => section.key === 'drifted')?.hiddenCount).toBe(1)
  })
})

describe('hasLockDifferences', () => {
  it('is false for an empty report', () => {
    expect(hasLockDifferences(makeReport())).toBe(false)
  })

  it('is true when any category is non-empty', () => {
    expect(hasLockDifferences(makeReport({ targets_changed: ['cursor-skill'] }))).toBe(true)
    expect(hasLockDifferences(makeReport({ drifted: [{ name: 'a', lock_hash: '1', local_hash: '2' }] }))).toBe(true)
  })
})
