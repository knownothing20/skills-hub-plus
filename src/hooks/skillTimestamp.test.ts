import { describe, expect, it } from 'vitest'
import { resolveTimestampKind } from './skillTimestamp'

describe('resolveTimestampKind', () => {
  it('两个时间戳相等时判为生成', () => {
    // 真实数据里 digital-human-video-editing 就是这种：11-25 21:39:58 装进来的
    expect(resolveTimestampKind({ created_at: 1795613998000, updated_at: 1795613998000 })).toBe(
      'created',
    )
  })

  it('更新过（updated 晚于 created）判为更新', () => {
    // word2ppt 实测：created 11-17 13:19:04 → updated 11-26 13:04:28
    expect(resolveTimestampKind({ created_at: 1795000000000, updated_at: 1795900000000 })).toBe(
      'updated',
    )
  })

  it('毫秒级差异仍算生成，不误判', () => {
    expect(resolveTimestampKind({ created_at: 1795613998000, updated_at: 1795613998500 })).toBe(
      'created',
    )
  })

  it('差距刚好超过 1 秒容差时才判为更新', () => {
    const base = 1795613998000
    expect(resolveTimestampKind({ created_at: base, updated_at: base + 1000 })).toBe('created')
    expect(resolveTimestampKind({ created_at: base, updated_at: base + 1001 })).toBe('updated')
  })

  it('字段缺失时退回更新，不凭空宣称「刚生成」', () => {
    expect(resolveTimestampKind({ created_at: 0, updated_at: 1795900000000 })).toBe('updated')
    expect(resolveTimestampKind({ created_at: 1795900000000, updated_at: 0 })).toBe('updated')
    expect(resolveTimestampKind({ created_at: 0, updated_at: 0 })).toBe('updated')
  })
})