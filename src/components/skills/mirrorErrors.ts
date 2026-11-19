/**
 * 「用原件覆盖母库」失败时，把后端错误码翻成人话。
 *
 * 后端统一用 `<CODE>|<detail>` 的形式报错，直接把原始字符串弹给用户等于让他
 * 读协议。这里只覆盖 refresh_skill_mirror 自己的错误码，其余原样透出，避免
 * 吞掉未知错误的关键信息。
 */
const MIRROR_ERRORS: Record<string, string> = {
  SKILL_NOT_FOUND: '找不到这条技能记录，请刷新列表后重试。',
  NO_SOURCE: '这条技能没有登记原件目录，无法用原件覆盖母库。',
  SOURCE_MISSING: '原件目录不存在，可能已被移动或删除。',
  SOURCE_EMPTY: '原件目录是空的。覆盖会把母库清空，已中止。',
  SAME_PATH: '原件与母库是同一个目录，无需覆盖。',
  UNSAFE_TARGET: '母库路径不在技能库根目录下，出于安全考虑已拒绝覆盖。',
}

export function describeMirrorFailure(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err)
  const [code, ...rest] = raw.split('|')
  const detail = rest.join('|').trim()
  const friendly = MIRROR_ERRORS[code]
  if (!friendly) {
    return `更新母库失败: ${raw}`
  }
  return detail ? `${friendly}（${detail}）` : friendly
}