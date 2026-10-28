import type { CSSProperties } from 'react'
import ampLogo from '@lobehub/icons-static-svg/icons/amp-color.svg'
import antigravityLogo from '@lobehub/icons-static-svg/icons/antigravity-color.svg'
import claudeCodeLogo from '@lobehub/icons-static-svg/icons/claudecode-color.svg'
import claudeLogo from '@lobehub/icons-static-svg/icons/claude-color.svg'
import clineLogo from '@lobehub/icons-static-svg/icons/cline.svg'
import codeBuddyLogo from '@lobehub/icons-static-svg/icons/codebuddy-color.svg'
import codexLogo from '@lobehub/icons-static-svg/icons/codex-color.svg'
import copilotLogo from '@lobehub/icons-static-svg/icons/copilot-color.svg'
import cursorLogo from '@lobehub/icons-static-svg/icons/cursor.svg'
import deepSeekLogo from '@lobehub/icons-static-svg/icons/deepseek-color.svg'
import doubaoLogo from '@lobehub/icons-static-svg/icons/doubao-color.svg'
import geminiCliLogo from '@lobehub/icons-static-svg/icons/geminicli-color.svg'
import gooseLogo from '@lobehub/icons-static-svg/icons/goose.svg'
import hermesAgentLogo from '@lobehub/icons-static-svg/icons/hermesagent.svg'
import inflectionLogo from '@lobehub/icons-static-svg/icons/inflection.svg'
import junieLogo from '@lobehub/icons-static-svg/icons/junie-color.svg'
import kiloCodeLogo from '@lobehub/icons-static-svg/icons/kilocode.svg'
import kimiLogo from '@lobehub/icons-static-svg/icons/kimi-color.svg'
import kiroLogo from '@lobehub/icons-static-svg/icons/kiro-color.svg'
import mcpLogo from '@lobehub/icons-static-svg/icons/mcp.svg'
import mistralLogo from '@lobehub/icons-static-svg/icons/mistral-color.svg'
import openClawLogo from '@lobehub/icons-static-svg/icons/openclaw-color.svg'
import openCodeLogo from '@lobehub/icons-static-svg/icons/opencode.svg'
import openHandsLogo from '@lobehub/icons-static-svg/icons/openhands-color.svg'
import qoderLogo from '@lobehub/icons-static-svg/icons/qoder-color.svg'
import qwenLogo from '@lobehub/icons-static-svg/icons/qwen-color.svg'
import rooCodeLogo from '@lobehub/icons-static-svg/icons/roocode.svg'
import traeLogo from '@lobehub/icons-static-svg/icons/trae-color.svg'
import windsurfLogo from '@lobehub/icons-static-svg/icons/windsurf.svg'
import zencoderLogo from '@lobehub/icons-static-svg/icons/zencoder-color.svg'
import workbuddyLogo from '../../assets/workbuddy.png'
import agentsLogo from '../../assets/logo-mark.svg'

import adalLogo from '../../assets/tools/adal.svg'
import augmentLogo from '../../assets/tools/augment.svg'
import codewhaleLogo from '../../assets/tools/codewhale.svg'
import commandCodeLogo from '../../assets/tools/command_code.svg'
import continueLogo from '../../assets/tools/continue.svg'
import copawLogo from '../../assets/tools/copaw.svg'
import crushLogo from '../../assets/tools/crush.svg'
import droidLogo from '../../assets/tools/droid.svg'
import iflowLogo from '../../assets/tools/iflow.svg'
import kodeLogo from '../../assets/tools/kode.svg'
import muxLogo from '../../assets/tools/mux.svg'
import neovateLogo from '../../assets/tools/neovate.svg'
import pochiLogo from '../../assets/tools/pochi.svg'
import zcodeLogo from '../../assets/tools/zcode.svg'

type ToolIconProps = {
  toolKey: string
  label: string
  avatar?: string | null
  className?: string
}

const logoByToolKey: Record<string, string> = {
  adal: adalLogo,
  agents: agentsLogo,
  amp: ampLogo,
  antigravity: antigravityLogo,
  augment: augmentLogo,
  autoclaw: openClawLogo,
  claude_code: claudeCodeLogo,
  clawdbot: openClawLogo,
  cline: clineLogo,
  codebuddy: codeBuddyLogo,
  codewhale: codewhaleLogo,
  codex: codexLogo,
  command_code: commandCodeLogo,
  continue: continueLogo,
  copaw: copawLogo,
  crush: crushLogo,
  cursor: cursorLogo,
  deepseek_harness: deepSeekLogo,
  doubao: doubaoLogo,
  doubao_work: doubaoLogo,
  droid: droidLogo,
  gemini_cli: geminiCliLogo,
  github_copilot: copilotLogo,
  goose: gooseLogo,
  hermes_agent: hermesAgentLogo,
  iflow_cli: iflowLogo,
  junie: junieLogo,
  kilo_code: kiloCodeLogo,
  kimi_cli: kimiLogo,
  kiro_cli: kiroLogo,
  kode: kodeLogo,
  mcpjam: mcpLogo,
  mistral_vibe: mistralLogo,
  moltbot: openClawLogo,
  mux: muxLogo,
  neovate: neovateLogo,
  openclaw: openClawLogo,
  openclaude: claudeLogo,
  opencode: openCodeLogo,
  openhands: openHandsLogo,
  pi: inflectionLogo,
  pochi: pochiLogo,
  qoder: qoderLogo,
  qoderwork: qoderLogo,
  qwen_code: qwenLogo,
  roo_code: rooCodeLogo,
  trae: traeLogo,
  trae_cn: traeLogo,
  windsurf: windsurfLogo,
  workbuddy: workbuddyLogo,
  workbuddy_ai: workbuddyLogo,
  zcode: zcodeLogo,
  zencoder: zencoderLogo,
}

const fallbackColorByToolKey: Record<string, string> = {
  zcode: '#8B5CF6',
  autoclaw: '#FF5722',
  agents: '#3B82F6',
  augment: '#6d5dfc',
  copaw: '#f26b3a',
  codewhale: '#1686d9',
  doubao_work: '#1E37FC',
  doubao: '#1E37FC',
  workbuddy: '#00BC90',
  workbuddy_ai: '#00BC90',
  command_code: '#171717',
  continue: '#f59e0b',
  crush: '#e83e8c',
  iflow_cli: '#5f62e9',
  kode: '#6f50e8',
  mux: '#1677ff',
  openclaude: '#d97757',
  pi: '#292524',
  neovate: '#15a37d',
  pochi: '#ff6b6b',
  adal: '#0f766e',
  droid: '#6366f1',
}

const getInitials = (label: string) =>
  label
    .split(/[\s_-]+/)
    .filter(Boolean)
    .map((part) => part[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

const ToolIcon = ({ toolKey, label, avatar, className = '' }: ToolIconProps) => {
  const logo = avatar || logoByToolKey[toolKey]
  const style = {
    '--tool-brand': fallbackColorByToolKey[toolKey] ?? '#64748b',
  } as CSSProperties

  return (
    <span
      className={`tool-brand-icon${logo ? ' has-logo' : ' fallback'}${avatar ? ' custom-avatar' : ''}${className ? ` ${className}` : ''}`}
      style={style}
      aria-hidden="true"
    >
      {logo ? <img src={logo} alt="" draggable={false} /> : getInitials(label)}
    </span>
  )
}

export default ToolIcon
