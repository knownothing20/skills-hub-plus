import type { ManagedSkill } from '../components/skills/types'

/**
 * 时间戳表达的到底是「生成」还是「更新」。
 *
 * 卡片上显示的 `skill.updated_at` 在两种情况下会变：
 * - 首次导入/安装：此时 `updated_at === created_at`，语义就是「生成」；
 * - 内容被刷新：`updated_at > created_at`，语义是「更新」。
 *
 * 只比较两个时间戳，不引入额外字段，因此对历史数据同样成立
 * （老记录只要没更新过，两者本来就相等）。
 *
 * 容差说明：`created_at` 与首次写入的 `updated_at` 在同一次事务里生成，
 * 正常情况下完全相等；但历史迁移/秒级截断可能留下几毫秒差。给 1 秒容差，
 * 避免把「刚生成」误判成「更新过」。
 */
export const CREATED_EQUALS_UPDATED_TOLERANCE_MS = 1000

export type TimeStampKind = 'created' | 'updated'

export function resolveTimestampKind(
  skill: Pick<ManagedSkill, 'created_at' | 'updated_at'>,
): TimeStampKind {
  const created = skill.created_at
  const updated = skill.updated_at
  // 任一缺失（0 或非数）时无法判断，退回「更新」——这是更常见的语义，
  // 也不会凭空宣称一个技能「刚生成」。
  if (!created || !updated) return 'updated'
  return updated - created > CREATED_EQUALS_UPDATED_TOLERANCE_MS
    ? 'updated'
    : 'created'
}