import { describe, expect, it } from 'vitest'
import { findOwnerTool, normalizePathKey, roleOfTarget, summarizeRoles } from './skillOwner'

describe('normalizePathKey', () => {
  it('treats separators and case as equivalent', () => {
    expect(normalizePathKey('C:\\Users\\Example\\.codex\\skills\\a')).toBe(
      'c:/users/example/.codex/skills/a',
    )
  })

  it('drops trailing separators', () => {
    expect(normalizePathKey('D:/skills/a/')).toBe(
      normalizePathKey('D:\\skills\\a'),
    )
  })

  it('returns an empty key for missing values', () => {
    expect(normalizePathKey(null)).toBe('')
    expect(normalizePathKey(undefined)).toBe('')
  })
})

describe('findOwnerTool', () => {
  it('returns the tool whose target path equals source_ref', () => {
    const owner = findOwnerTool({
      source_type: 'local',
      source_ref: 'C:\\Users\\example\\.workbuddy\\skills\\腾讯文档',
      targets: [
        {
          tool: 'workbuddy',
          target_path: 'C:/Users/Example/.workbuddy/skills/腾讯文档',
        },
        { tool: 'codex', target_path: 'C:\\Users\\example\\.codex\\skills\\腾讯文档' },
      ],
    })
    expect(owner).toBe('workbuddy')
  })

  it('ignores a link target that merely contains the name', () => {
    const owner = findOwnerTool({
      source_type: 'local',
      source_ref: 'C:\\a\\skills\\demo',
      targets: [{ tool: 'codex', target_path: 'C:\\different\\skills\\demo-extra' }],
    })
    expect(owner).toBeNull()
  })

  it('returns null for centrally managed Skills', () => {
    const owner = findOwnerTool({
      source_type: 'managed',
      source_ref: null,
      targets: [{ tool: 'codex', target_path: 'C:\\a\\skills\\demo' }],
    })
    expect(owner).toBeNull()
  })

  it('returns null when source_ref is empty', () => {
    const owner = findOwnerTool({
      source_type: 'local',
      source_ref: '   ',
      targets: [{ tool: 'codex', target_path: 'C:\\a\\skills\\demo' }],
    })
    expect(owner).toBeNull()
  })
})

describe('roleOfTarget', () => {
  it('prefers the backend resolved role', () => {
    expect(roleOfTarget({ tool: 'codex', role: 'owner', mode: 'copy' })).toBe('owner')
  })

  it('falls back to the recorded mode for older payloads', () => {
    expect(roleOfTarget({ tool: 'codex', mode: 'copy' })).toBe('copy')
    expect(roleOfTarget({ tool: 'codex', mode: 'junction' })).toBe('link_to_central')
  })

  it('treats a missing target as broken', () => {
    expect(roleOfTarget(null)).toBe('missing')
    expect(roleOfTarget(undefined)).toBe('missing')
  })
})

describe('summarizeRoles', () => {
  it('groups owner, links, copies and broken entries', () => {
    const summary = summarizeRoles([
      { tool: 'doubao_work', role: 'owner' },
      { tool: 'codex', role: 'link_to_owner' },
      { tool: 'workbuddy', role: 'link_to_central' },
      { tool: 'zcode', role: 'copy' },
      { tool: 'cline', role: 'missing' },
    ])
    expect(summary).toEqual({ owner: 1, links: 2, copies: 1, broken: 1 })
  })

  it('returns zeros for an empty list', () => {
    expect(summarizeRoles([])).toEqual({ owner: 0, links: 0, copies: 0, broken: 0 })
  })
})
