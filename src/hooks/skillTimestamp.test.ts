import { describe, expect, it } from 'vitest'
import { resolveDisplayTime, resolveTimestampKind } from './skillTimestamp'

describe('resolveTimestampKind', () => {
  it('未被编辑过判为生成', () => {
    // 真实数据：digital-human-video-editing 只有一条建基线时落的历史，
    // created_at == updated_at，装进来之后没动过。
    expect(resolveTimestampKind({ content_edited: false })).toBe('created')
  })

  it('被编辑过判为更新', () => {
    // 真实数据：xiaohongshu-ai-creator 11-26 21:02 改过内容，
    // 历史里有多条指纹，但 created_at == updated_at（旧代码因此判错）。
    expect(resolveTimestampKind({ content_edited: true })).toBe('updated')
  })

  it('不看时间戳差值，只看后端给的编辑标记', () => {
    // 这是回归防线：曾经用 updated_at - created_at 判断，
    // 而 updated_at 不随内容变更更新，导致真实编辑全被漏掉。
    // 下面两种「时间戳说了 A、编辑标记说了 B」的组合，必须以后者为准。
    expect(resolveTimestampKind({ content_edited: false })).toBe('created')
    expect(resolveTimestampKind({ content_edited: true })).toBe('updated')
  })
})

describe('resolveDisplayTime', () => {
  it('正常取内容变更时间', () => {
    expect(
      resolveDisplayTime({
        content_changed_at: 1795683129000,
        created_at: 1795613998000,
      }),
    ).toBe(1795683129000)
  })

  it('内容变更时间缺失时退回入库时间', () => {
    expect(resolveDisplayTime({ content_changed_at: 0, created_at: 1795613998000 })).toBe(
      1795613998000,
    )
  })

  it('字段为 0 或非数时不返回 0，避免显示成 1970', () => {
    expect(resolveDisplayTime({ content_changed_at: 0, created_at: 1795613998000 })).not.toBe(0)
  })
})