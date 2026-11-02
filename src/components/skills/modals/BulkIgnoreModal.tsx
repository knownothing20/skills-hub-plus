import { memo } from 'react'
import { ShieldOff } from 'lucide-react'
import type { TFunction } from 'i18next'

type BulkIgnoreModalProps = {
  open: boolean
  loading: boolean
  skillNames: string[]
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const BulkIgnoreModal = ({
  open,
  loading,
  skillNames,
  onRequestClose,
  onConfirm,
  t,
}: BulkIgnoreModalProps) => {
  if (!open) return null

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div
        className="modal modal-delete bulk-delete-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-body delete-body">
          <div className="delete-title" style={{ color: 'var(--text-color, #1f2937)' }}>
            <ShieldOff size={20} style={{ color: '#d97706' }} />
            {t('bulk.ignoreTitle', {
              count: skillNames.length,
              defaultValue: `批量屏蔽技能 (${skillNames.length} 个)`,
            })}
          </div>
          <div className="delete-desc">
            {t('bulk.ignoreBody', {
              defaultValue:
                '选中的技能将从 Skills Hub 解除托管并加入屏蔽名单。各宿主软件（如 Antigravity / Cursor 等）中的技能将保留为独立完整副本，后续扫描不会再提示导入。',
            })}
          </div>
          <div className="bulk-delete-list">
            {skillNames.slice(0, 6).map((name, index) => (
              <span key={`${name}-${index}`}>{name}</span>
            ))}
            {skillNames.length > 6 ? (
              <span>{t('bulk.moreSelected', { count: skillNames.length - 6 })}</span>
            ) : null}
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
            <span>
              {t('bulk.ignoreConfirm', {
                count: skillNames.length,
                defaultValue: `确认屏蔽 (${skillNames.length} 个)`,
              })}
            </span>
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(BulkIgnoreModal)
