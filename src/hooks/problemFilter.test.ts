import { describe, it, expect } from 'vitest'
import { resolveProblemFilter } from './problemFilter'

describe('resolveProblemFilter', () => {
  it('未启用时不过滤', () => {
    expect(resolveProblemFilter(false, new Set(['a']))).toBeNull()
  })

  it('启用时用当前问题集合过滤', () => {
    const set = new Set(['a', 'b'])
    expect(resolveProblemFilter(true, set)).toBe(set)
  })

  it('问题被修完后自动放开过滤（不显示 0 个的空列表）', () => {
    expect(resolveProblemFilter(true, new Set())).toBeNull()
  })

  it('问题集合变化时结论跟着变，不保留旧名单', () => {
    const before = new Set(['compute-hub', 'grill-me'])
    expect(resolveProblemFilter(true, before)?.size).toBe(2)
    const after = new Set<string>()
    expect(resolveProblemFilter(true, after)).toBeNull()
  })
})