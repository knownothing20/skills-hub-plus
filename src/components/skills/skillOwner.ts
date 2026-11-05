/**
 * Work out which tool owns a Skill's original content.
 *
 * A Skill recorded with `source_type = "local"` keeps its original inside a
 * software folder (`source_ref`); the central copy is only a mirror. That same
 * tool also has a sync target whose `target_path` is that very folder, so the
 * owner can be derived on the client without another backend call.
 */

export type OwnerLookupTarget = {
  tool: string
  target_path?: string | null
  role?: string | null
  mode?: string | null
}

/** How a sync target relates to the Skill's authoritative content. */
export type TargetRole =
  | 'owner'
  | 'link_to_owner'
  | 'link_to_central'
  | 'copy'
  | 'link_other'
  | 'missing'

export type RoleSummary = {
  owner: number
  links: number
  copies: number
  broken: number
}

const KNOWN_ROLES: TargetRole[] = [
  'owner',
  'link_to_owner',
  'link_to_central',
  'copy',
  'link_other',
  'missing',
]

/**
 * Role of one target. Prefers the backend-resolved `role`; falls back to the
 * recorded mode when an older payload arrives without it.
 */
export function roleOfTarget(target?: OwnerLookupTarget | null): TargetRole {
  if (!target) return 'missing'
  const role = target.role?.trim() as TargetRole | undefined
  if (role && KNOWN_ROLES.includes(role)) return role
  return target.mode === 'copy' ? 'copy' : 'link_to_central'
}

export function isBrokenRole(role: TargetRole): boolean {
  return role === 'missing' || role === 'link_other'
}

export function summarizeRoles(
  targets: OwnerLookupTarget[],
): RoleSummary {
  const summary: RoleSummary = { owner: 0, links: 0, copies: 0, broken: 0 }
  for (const target of targets) {
    const role = roleOfTarget(target)
    if (role === 'owner') summary.owner += 1
    else if (role === 'copy') summary.copies += 1
    else if (role === 'link_to_owner' || role === 'link_to_central') {
      summary.links += 1
    } else summary.broken += 1
  }
  return summary
}

export type OwnerLookupSkill = {
  source_type: string
  source_ref?: string | null
  targets: OwnerLookupTarget[]
}

/** Case- and separator-insensitive key for comparing Windows paths. */
export function normalizePathKey(value?: string | null): string {
  return (value ?? '')
    .replace(/\\/g, '/')
    .replace(/\/+$/, '')
    .toLowerCase()
}

/**
 * Tool key whose folder holds the original, or `null` when the Skill has no
 * recorded owner (its central copy is the authority) or the owner cannot be
 * matched to a target.
 */
export function findOwnerTool(skill: OwnerLookupSkill): string | null {
  if (skill.source_type !== 'local') return null
  const reference = normalizePathKey(skill.source_ref)
  if (!reference) return null
  const match = (skill.targets ?? []).find(
    (target) => normalizePathKey(target.target_path) === reference,
  )
  return match ? match.tool : null
}
