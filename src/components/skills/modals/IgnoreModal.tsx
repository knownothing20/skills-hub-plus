import { memo } from 'react'
import { ShieldOff } from 'lucide-react'
import type { TFunction } from 'i18next'

type IgnoreModalProps = {
  open: boolean
  loading: boolean
  skillName: string | null
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const IgnoreModal = ({
  open,
  loading,
  skillName,
  onRequestClose,
  onConfirm,
  t,
}: IgnoreModalProps) => {
  if (!open) return null

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div
        className="modal modal-delete"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-body delete-body">
          <div className="delete-title" style={{ color: 'var(--text-color, #1f2937)' }}>
            <ShieldOff size={20} style={{ color: '#d97706' }} />
            {t('ignoreModal.title', { defaultValue: '屏蔽技能' })}
          </div>
          <div className="delete-desc">
            {skillName ? (
              <>
                {t('ignoreModal.confirmPrefix', { defaultValue: '确定要屏蔽技能 “' })}
                <strong>{skillName}</strong>
                {t('ignoreModal.confirmSuffix', { defaultValue: '” 吗？' })}
              </>
            ) : (
              t('ignoreModal.descDefault', { defaultValue: '确定要屏蔽该技能吗？' })
            )}
          </div>
          <div className="delete-warning">
            <ul>
              <li>
                {t('ignoreModal.warningDetach', {
                  defaultValue: '解除托管：从 Skills Hub 列表中移除，不再统一同步',
                })}
              </li>
              <li>
                {t('ignoreModal.warningKeepLocal', {
                  defaultValue: '保留软件副本：宿主工具（如 Antigravity / Cursor 等）中的技能将转为独立文件，不影响现有使用',
                })}
              </li>
              <li>
                {t('ignoreModal.warningIgnoreList', {
                  defaultValue: '加入屏蔽名单：后续自动扫描或导入新技能时将自动忽略此技能',
                })}
              </li>
              <li>
                {t('ignoreModal.warningRecover', {
                  defaultValue: '可随时恢复：后续可以在“设置 -> 屏蔽列表”中随时查看并解除屏蔽',
                })}
              </li>
            </ul>
          </div>
        </div>
        <div className="modal-footer space-between">
          <button
            className="btn btn-secondary"
            onClick={onRequestClose}
            disabled={loading}
          >
            {t('cancel')}
          </button>
          <button
            className="btn btn-secondary"
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '6px',
              backgroundColor: '#fef3c7',
              borderColor: '#f59e0b',
              color: '#92400e',
              fontWeight: 500,
            }}
            onClick={onConfirm}
            disabled={loading}
          >
            <ShieldOff size={15} />
            <span>{t('ignoreModal.confirmButton', { defaultValue: '确认屏蔽' })}</span>
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(IgnoreModal)
