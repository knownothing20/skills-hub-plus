import { memo } from 'react'
import type { TFunction } from 'i18next'

import { buildLockDiffSections, hasLockDifferences } from '../skillsLockReport'
import type { LockDiffReportDto } from '../types'

type LockDiffModalProps = {
  open: boolean
  report: LockDiffReportDto | null
  onRequestClose: () => void
  t: TFunction
}

/**
 * Plain, read-only view of a skills.lock comparison: four sections with
 * counts and a short preview of names. v1 never installs or deletes.
 */
const LockDiffModal = ({ open, report, onRequestClose, t }: LockDiffModalProps) => {
  if (!open || !report) return null

  const sections = buildLockDiffSections(report)

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div
        className="modal modal-lg lock-diff-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-header">
          <div className="modal-title">{t('lock.reportTitle')}</div>
        </div>
        <div className="modal-body">
          <div className="lock-report-path mono">{t('lock.reportPath', { path: report.lock_path })}</div>
          <div className="lock-report-hint">{t('lock.reportHint')}</div>

          {!hasLockDifferences(report) && (
            <div className="lock-report-empty">{t('lock.noDifferences')}</div>
          )}

          {sections.map((section) => (
            <div key={section.key} className="lock-report-section">
              <div className="lock-report-section-title">
                {t(`lock.sections.${section.key}`, { count: section.total })}
              </div>
              {section.entries.length > 0 ? (
                <ul className="lock-report-list mono">
                  {section.entries.map((name) => (
                    <li key={`${section.key}-${name}`}>{name}</li>
                  ))}
                </ul>
              ) : (
                <div className="lock-report-none">{t('lock.emptySection')}</div>
              )}
              {section.hiddenCount > 0 && (
                <div className="lock-report-more">
                  {t('lock.moreEntries', { count: section.hiddenCount })}
                </div>
              )}
            </div>
          ))}
        </div>
        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onRequestClose}>
            {t('cancel')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(LockDiffModal)
