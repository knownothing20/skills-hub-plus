import type { ManagedSkill } from '../components/skills/types'

/**
 * 时间戳表达的到底是「生成」还是「更新」。
 *
 * ## 为什么不再比较 created_at / updated_at
 *
 * 早先这里是 `updated_at > created_at` 判「更新」，前提假设是「内容一改，
 * `updated_at` 就动」。这个假设是错的：`skills.updated_at` 只在安装、
 * 启用/停用、设为母版、重建镜像这类**元数据或结构**操作时写入；真正的内容
 * 变更走 `peer_sync` 的采纳路径，那条路径只写 `skill_baselines` 与
 * `skill_baseline_history`，从不碰 `updated_at`。
 *
 * 结果就是所有真实编辑都看不见。2026-11 的小红书技能：入库 11-25 21:39，
 * 内容 11-26 21:02 改过，`created_at == updated_at`，被判成「生成」。
 *
 * ## 现在的口径
 *
 * 后端 `ManagedSkillDto` 直接下发两个字段，判定不需要前端再猜：
 * - `content_changed_at`：内容最后变更时间（有历史取历史最新，否则取入库时间）
 * - `content_edited`：是否真的被编辑过（首次建基线那条历史不算编辑）
 *
 * 前端只读 `content_edited`。保留 `resolveTimestampKind` 这个函数名，是为了
 * 让调用处不用改；参数收窄成只依赖这两个字段，语义更清楚。
 */
export type TimeStampKind = 'created' | 'updated'

export function resolveTimestampKind(
  skill: Pick<ManagedSkill, 'content_edited'>,
): TimeStampKind {
  return skill.content_edited ? 'updated' : 'created'
}

/**
 * 卡片上该显示的时间。
 *
 * 语义是「内容最后变更时间」，不是 `updated_at`（那个会被开关切换污染，
 * 也不反映内容编辑）。缺失时退回 `created_at`，至少显示的不是错的时间。
 */
export function resolveDisplayTime(
  skill: Pick<ManagedSkill, 'content_changed_at' | 'created_at'>,
): number {
  const changed = skill.content_changed_at
  if (typeof changed === 'number' && changed > 0) return changed
  return skill.created_at
}