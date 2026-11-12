import { renderToStaticMarkup } from 'react-dom/server'
import type { TFunction } from 'i18next'
import { describe, expect, it, vi } from 'vitest'
import SkillCard from './SkillCard'
import type { ManagedSkill, ToolOption } from './types'

const translate = ((key: string) => key) as TFunction

const createSkill = (updateable: boolean): ManagedSkill => ({
  id: 'example',
  name: 'example',
  description: 'Example Skill',
  source_type: 'git',
  source_ref: 'https://example.com/repo.git',
  has_external_source: true,
  central_path: '/Users/example/.agents/skills/example',
  created_at: 1,
  updated_at: 1,
  enabled: true,
  updateable,
  status: 'ok',
  tags: [],
  targets: [],
})

const renderCard = (skill: ManagedSkill, installedTools: ToolOption[] = []) =>
  renderToStaticMarkup(
    <SkillCard
      skill={skill}
      installedTools={installedTools}
    loading={false}
    bulkMode={false}
    bulkSelected={false}
    getGithubInfo={() => null}
    getSkillSourceLabel={() => 'source'}
    formatRelative={() => 'now'}
    onUpdate={vi.fn()}
    onDelete={vi.fn()}
    onToggleEnabled={vi.fn()}
    onToggleTool={vi.fn()}
    onOpenScope={vi.fn()}
    onOpenDetail={vi.fn()}
    onEditTags={vi.fn()}
    onToggleBulkSelection={vi.fn()}
    getSkillScope={() => 'global'}
      getSkillProjects={() => []}
      t={translate}
    />,
  )

describe('SkillCard update action', () => {
  it('disables update when the backend marks a Git-looking skill as not updateable', () => {
    const markup = renderCard(createSkill(false))

    expect(markup).toMatch(
      /<button type="button" disabled="" aria-label="updateSourceUnavailable"[^>]*>/,
    )
  })

  it('enables update when the backend marks the skill as updateable', () => {
    const markup = renderCard(createSkill(true))

    expect(markup).toContain('aria-label="update" title="update"')
    expect(markup).not.toMatch(
      /<button type="button" disabled="" aria-label="update"[^>]*>/,
    )
  })

  it('passes backend Skill icon metadata through to the icon renderer', () => {
    const skill = {
      ...createSkill(true),
      icon_data_url: 'data:image/png;base64,iVBORw0KGgo=',
      brand_color: '#123ABC',
    }
    const markup = renderCard(skill)

    expect(markup).toContain('data-icon-source="metadata"')
    expect(markup).toContain('--skill-icon-color:#123ABC')
    expect(markup).toContain('src="data:image/png;base64,iVBORw0KGgo="')
  })

  it('renders ignore button when onIgnore prop is provided', () => {
    const markup = renderToStaticMarkup(
      <SkillCard
        skill={createSkill(true)}
        installedTools={[]}
        loading={false}
        bulkMode={false}
        bulkSelected={false}
        getGithubInfo={() => null}
        getSkillSourceLabel={() => 'source'}
        formatRelative={() => 'now'}
        onUpdate={vi.fn()}
        onDelete={vi.fn()}
        onIgnore={vi.fn()}
        onToggleEnabled={vi.fn()}
        onToggleTool={vi.fn()}
        onOpenScope={vi.fn()}
        onOpenDetail={vi.fn()}
        onEditTags={vi.fn()}
        onToggleBulkSelection={vi.fn()}
        getSkillScope={() => 'global'}
        getSkillProjects={() => []}
        t={translate}
      />,
    )

    expect(markup).toContain('aria-label="ignoreSkill"')
    expect(markup).toContain('title="ignoreSkillTooltip"')
  })
})

describe('SkillCard tool target description', () => {
  // 原件所在的工具（source_ref 指向它）在记录里 mode='copy'，但 role='owner'。
  // 曾经用 role 反推形态，导致原件被说成「软链接」，而磁盘上它是实体目录。
  const ownerCopySkill: ManagedSkill = {
    ...createSkill(false),
    source_type: 'local',
    source_ref: 'C:/Users/leon/.codex/skills/pinokio',
    targets: [
      {
        tool: 'codex',
        scope: 'global',
        mode: 'copy',
        status: 'ok',
        target_path: 'C:/Users/leon/.codex/skills/pinokio',
        role: 'owner',
      },
    ],
  }

  const codex: ToolOption = { id: 'codex', label: 'Codex' }

  it('calls the owning tool the local source instead of a symlink', () => {
    const markup = renderCard(ownerCopySkill, [codex])

    expect(markup).toContain('原件（本地源）')
    expect(markup).not.toContain('is-symlink')
    expect(markup).toContain('is-owner')
  })

  it('keeps a real copy target described as a copy', () => {
    const copied: ManagedSkill = {
      ...createSkill(false),
      targets: [
        {
          tool: 'codex',
          scope: 'global',
          mode: 'copy',
          status: 'ok',
          target_path: '/elsewhere/pinokio',
          role: 'copy',
        },
      ],
    }

    const markup = renderCard(copied, [codex])

    expect(markup).toContain('is-copy')
    expect(markup).not.toContain('is-owner')
  })

  it('renders a legacy junction target as a solid copy', () => {
    // 2.0.1 起同步只有复制一种方式：历史链接记录在界面上按实体副本展示，
    // 由对等同步在下一轮将其转为实体目录。
    const linked: ManagedSkill = {
      ...createSkill(false),
      targets: [
        {
          tool: 'codex',
          scope: 'global',
          mode: 'junction',
          status: 'ok',
          target_path: '/elsewhere/pinokio',
          role: 'link_to_central',
        },
      ],
    }

    const markup = renderCard(linked, [codex])

    expect(markup).toContain('is-copy')
    expect(markup).not.toContain('is-symlink')
  })
})
