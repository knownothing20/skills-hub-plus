import { describe, expect, it } from 'vitest'
import { describeMirrorFailure } from './mirrorErrors'

describe('describeMirrorFailure', () => {
  it('把已知错误码翻成人话并保留路径细节', () => {
    const message = describeMirrorFailure(
      new Error('SOURCE_EMPTY|C:\\Users\\Example\\.codex\\skills\\demo'),
    )
    expect(message).toContain('原件目录是空的')
    expect(message).toContain('C:\\Users\\Example\\.codex\\skills\\demo')
  })

  it('没有细节时不留空的括号', () => {
    const message = describeMirrorFailure(new Error('NO_SOURCE|abc'))
    // NO_SOURCE 会带 skill id；这里验证拼接形式不出现空括号
    expect(message).not.toContain('（）')
    expect(message).not.toContain('()')
  })

  it('未知错误码原样透出，不吞掉信息', () => {
    const message = describeMirrorFailure(new Error('SOME_NEW_CODE|detail-here'))
    expect(message).toContain('SOME_NEW_CODE|detail-here')
  })

  it('非 Error 输入也能处理', () => {
    expect(describeMirrorFailure('SOURCE_MISSING|x')).toContain('原件目录不存在')
  })

  it('拒绝把母库写到库根之外时有明确提示', () => {
    expect(describeMirrorFailure(new Error('UNSAFE_TARGET|D:\\elsewhere\\demo'))).toContain(
      '不在技能库根目录下',
    )
  })
})