/**
 * 「只显示有问题的技能」的过滤决策。
 *
 * 这里必须是纯函数、并且只依赖当前的问题集合，不能缓存点击那一刻的名单：
 * 后台启动刷新会在界面启动后几秒内把落后副本修好，若名单被冻结，列表就会长期
 * 挂着一批已经修好的技能（横幅写着「6 个」但每一项其实都没问题），只有手动
 * 刷新才恢复。抽出来单独测，免得以后又被改回快照语义。
 */
export function resolveProblemFilter(
  enabled: boolean,
  problemNames: Set<string>,
): Set<string> | null {
  if (!enabled) return null
  // 问题已全部修完：自动放开过滤，而不是显示一个「0 个」的空列表。
  if (problemNames.size === 0) return null
  return problemNames
}