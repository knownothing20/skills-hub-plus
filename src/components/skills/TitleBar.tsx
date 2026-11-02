import { memo, type PointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { TFunction } from 'i18next'

type TitleBarProps = {
  appVersion: string
  t: TFunction
}

const startWindowDrag = (event: PointerEvent<HTMLElement>) => {
  if (event.button !== 0 || !event.isPrimary) return
  const target = event.target as HTMLElement
  if (target.closest('button, input, select, textarea, a, [role="button"]')) return
  event.preventDefault()
  void getCurrentWindow().startDragging().catch(() => undefined)
}

const TitleBar = ({ appVersion, t }: TitleBarProps) => (
  <header
    className="window-titlebar"
    data-tauri-drag-region
    onPointerDown={startWindowDrag}
  >
    <div className="traffic-lights" aria-hidden="true" data-tauri-drag-region>
      <span className="traffic-light red" data-tauri-drag-region />
      <span className="traffic-light yellow" data-tauri-drag-region />
      <span className="traffic-light green" data-tauri-drag-region />
    </div>
    <strong className="titlebar-heading" data-tauri-drag-region>
      {t('appName')}
    </strong>
    {appVersion ? (
      <div className="titlebar-version-status" data-tauri-drag-region>
        <span data-tauri-drag-region>v{appVersion}</span>
      </div>
    ) : null}
  </header>
)

export default memo(TitleBar)
