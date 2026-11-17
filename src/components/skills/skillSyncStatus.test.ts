import { describe, expect, it } from 'vitest'
import {
  getFullySyncedTools,
  getSkillSyncState,
  getToolSyncState,
} from './skillSyncStatus'
import type { ManagedSkill } from './types'

type Target = ManagedSkill['targets'][number]

const target = (
  status: string,
  tool = 'codex',
  scope: 'global' | 'project' = 'global',
): Target => ({
  tool,
  scope,
  mode: 'copy',
  status,
  target_path: `/tmp/${tool}`,
})

describe('getSkillSyncState', () => {
  it('distinguishes disabled, idle, healthy, partial, and failed skills', () => {
    expect(getSkillSyncState({ enabled: false, status: 'ok', targets: [target('ok')] })).toBe('disabled')
    expect(getSkillSyncState({ enabled: true, status: 'ok', targets: [] })).toBe('idle')
    expect(getSkillSyncState({ enabled: true, status: 'ok', targets: [target('ok')] })).toBe('healthy')
    expect(
      getSkillSyncState({ enabled: true, status: 'ok', targets: [target('ok'), target('error', 'cursor')] }),
    ).toBe('partial')
    expect(getSkillSyncState({ enabled: true, status: 'ok', targets: [target('error')] })).toBe('failed')
  })

  it('prioritizes a source update error over healthy sync targets', () => {
    expect(
      getSkillSyncState({ enabled: true, status: 'error', targets: [target('ok')] }),
    ).toBe('source-error')
  })

  it('ignores disabled targets when calculating health', () => {
    expect(
      getSkillSyncState({ enabled: true, status: 'ok', targets: [target('ok'), target('disabled', 'cursor')] }),
    ).toBe('healthy')
  })
})

describe('getToolSyncState', () => {
  it('uses only matching active targets in the selected scope', () => {
    const skill = {
      targets: [
        target('error'),
        target('ok', 'codex', 'project'),
        target('disabled', 'cursor'),
      ],
    }
    expect(getToolSyncState(skill, 'codex', 'global')).toBe('failed')
    expect(getToolSyncState(skill, 'codex', 'project')).toBe('synced')
    expect(getToolSyncState(skill, 'cursor', 'global')).toBe('not-synced')
  })

  it('reports partial when project targets have mixed results', () => {
    const skill = {
      targets: [target('ok', 'codex', 'project'), target('error', 'codex', 'project')],
    }
    expect(getToolSyncState(skill, 'codex', 'project')).toBe('partial')
  })

  it('returns only fully synced tools from the current tool list', () => {
    const skill = {
      targets: [
        target('ok', 'codex'),
        target('error', 'cursor'),
        target('ok', 'legacy-tool'),
      ],
    }
    const tools = [
      { id: 'codex', label: 'Codex' },
      { id: 'cursor', label: 'Cursor' },
    ]

    expect(getFullySyncedTools(skill, tools, 'global')).toEqual([
      { id: 'codex', label: 'Codex' },
    ])
  })
})

describe('targets whose copy is gone', () => {
  // 卸载软件后，数据库仍留着 status=ok，但后端已把 role 标成 missing。
  // 这类记录绝不能再显示成「已同步」，否则用户看不到问题、也点不掉它。
  const missingTarget = (tool: string): Target => ({
    tool,
    scope: 'global',
    mode: 'copy',
    status: 'ok',
    role: 'missing',
    target_path: `/gone/${tool}`,
  })

  it('does not count a vanished copy as synced', () => {
    const skill = { targets: [missingTarget('doubao')] }
    expect(getToolSyncState(skill, 'doubao', 'global')).toBe('failed')
  })

  it('keeps a skill with only vanished copies out of the healthy bucket', () => {
    expect(
      getSkillSyncState({ enabled: true, status: 'ok', targets: [missingTarget('doubao')] }),
    ).toBe('failed')
  })

  it('still reports a live copy as synced', () => {
    const live: Target = {
      tool: 'codex',
      scope: 'global',
      mode: 'copy',
      status: 'ok',
      role: 'copy',
      target_path: '/live/codex',
    }
    expect(getSkillSyncState({ enabled: true, status: 'ok', targets: [live] })).toBe('healthy')
  })
})
