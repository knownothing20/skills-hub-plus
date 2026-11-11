import { useCallback, useEffect, useMemo, useState } from 'react'
import Markdown from 'react-markdown'
import { ChevronRight, File, Folder, RefreshCw } from 'lucide-react'
import { invokeTauri } from '../../hooks/useTauriInvoke'

type CentralSkillSummary = {
  name: string
  path: string
  file_count: number
  size_bytes: number
  updated_at: number
  source_ref?: string | null
}

type LibraryFileNode = {
  name: string
  rel_path: string
  is_dir: boolean
  size_bytes: number
  children: LibraryFileNode[]
}

type Props = {
  onOpenFolder: (path: string) => void
}

function formatSize(bytes: number): string {
  if (bytes <= 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

const MARKDOWN_EXT = /\.(md|markdown)$/i

/// 把 markdown 顶部的 --- 元数据块单独摘出来：
/// 直接交给 Markdown 渲染会把它当成正文标题，看起来像是文档坏了。
function splitFrontmatter(text: string): { meta: string; body: string } {
  if (!text.startsWith('---')) return { meta: '', body: text }
  const end = text.indexOf('\n---', 3)
  if (end === -1) return { meta: '', body: text }
  const meta = text.slice(3, end).trim()
  const body = text.slice(end + 4).replace(/^\r?\n/, '')
  return { meta, body }
}

export default function CentralLibraryView({ onOpenFolder }: Props) {
  const [skills, setSkills] = useState<CentralSkillSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<string | null>(null)
  const [tree, setTree] = useState<LibraryFileNode[]>([])
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})
  const [activeFile, setActiveFile] = useState<string | null>(null)
  const [fileContent, setFileContent] = useState<string>('')
  const [fileError, setFileError] = useState<string | null>(null)
  const [fileLoading, setFileLoading] = useState(false)

  const loadSkills = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const list = await invokeTauri<CentralSkillSummary[]>('list_central_library')
      setSkills(list || [])
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void loadSkills()
  }, [loadSkills])

  const loadTree = useCallback(async (skillName: string) => {
    setSelected(skillName)
    setTree([])
    setActiveFile(null)
    setFileContent('')
    setFileError(null)
    try {
      const nodes = await invokeTauri<LibraryFileNode[]>('list_library_files', {
        skillName,
        maxDepth: 3,
      })
      setTree(nodes || [])
    } catch (err) {
      setError(String(err))
    }
  }, [])

  const openFile = useCallback(
    async (skillName: string, relPath: string) => {
      setActiveFile(relPath)
      setFileLoading(true)
      setFileError(null)
      try {
        const text = await invokeTauri<string>('read_library_file', { skillName, relPath })
        setFileContent(text)
      } catch (err) {
        setFileContent('')
        setFileError(String(err))
      } finally {
        setFileLoading(false)
      }
    },
    [],
  )

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return skills
    return skills.filter((s) => s.name.toLowerCase().includes(q))
  }, [skills, query])

  const current = skills.find((s) => s.name === selected)
  const totalSize = skills.reduce((sum, s) => sum + s.size_bytes, 0)

  const renderNodes = (nodes: LibraryFileNode[], depth: number) =>
    nodes.map((node) => {
      const isCollapsed = collapsed[node.rel_path]
      if (node.is_dir) {
        return (
          <div key={node.rel_path}>
            <button
              type="button"
              onClick={() =>
                setCollapsed((prev) => ({ ...prev, [node.rel_path]: !prev[node.rel_path] }))
              }
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '5px',
                width: '100%',
                padding: '4px 6px',
                paddingLeft: 6 + depth * 12,
                background: 'transparent',
                border: 0,
                borderRadius: '5px',
                cursor: 'pointer',
                textAlign: 'left',
                fontSize: '12px',
                color: 'var(--text-secondary)',
              }}
            >
              <ChevronRight
                size={12}
                style={{
                  transform: isCollapsed ? 'none' : 'rotate(90deg)',
                  transition: 'transform .12s ease',
                  flex: '0 0 12px',
                }}
              />
              <Folder size={13} />
              <span>{node.name}</span>
            </button>
            {!isCollapsed && renderNodes(node.children, depth + 1)}
          </div>
        )
      }
      const active = activeFile === node.rel_path
      return (
        <button
          key={node.rel_path}
          type="button"
          onClick={() => void openFile(selected ?? '', node.rel_path)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '5px',
            width: '100%',
            padding: '4px 6px',
            paddingLeft: 6 + depth * 12 + 17,
            background: active ? 'var(--bg-element)' : 'transparent',
            border: 0,
            borderRadius: '5px',
            cursor: 'pointer',
            textAlign: 'left',
            fontSize: '12px',
            color: active ? 'var(--text-primary)' : 'var(--text-secondary)',
          }}
        >
          <File size={13} />
          <span
            title={node.name}
            style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
          >
            {node.name}
          </span>
          <span style={{ marginLeft: 'auto', fontSize: '10px', color: 'var(--text-tertiary)' }}>
            {formatSize(node.size_bytes)}
          </span>
        </button>
      )
    })

  return (
    <div className="central-library" style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '12px',
          padding: '10px 4px 14px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'baseline', gap: '10px', minWidth: 0 }}>
          <span style={{ fontSize: '15px', fontWeight: 500 }}>母库</span>
          <span
            style={{
              fontSize: '11px',
              color: 'var(--text-tertiary)',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {skills[0]?.path ? skills[0].path.replace(/[\\/][^\\/]+$/, '') : ''}
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flex: '0 0 auto' }}>
          <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
            {skills.length} 个技能 · {formatSize(totalSize)}
          </span>
          <button
            type="button"
            className="btn btn-secondary"
            style={{ fontSize: '11px', padding: '4px 10px', height: 'auto' }}
            onClick={() => void loadSkills()}
            title="重新读取母库"
          >
            <RefreshCw size={12} />
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            style={{ fontSize: '11px', padding: '4px 10px', height: 'auto' }}
            onClick={() => {
              const dir = skills[0]?.path
              if (dir) onOpenFolder(dir.replace(/[\\/][^\\/]+$/, ''))
            }}
          >
            在文件夹中打开
          </button>
        </div>
      </div>

      {error && (
        <div style={{ fontSize: '12px', color: 'var(--status-error)', padding: '8px 0' }}>{error}</div>
      )}

      <div style={{ display: 'flex', flex: 1, minHeight: 0, gap: '12px' }}>
        <div
          style={{
            width: '230px',
            flex: '0 0 230px',
            display: 'flex',
            flexDirection: 'column',
            minHeight: 0,
            border: '1px solid var(--border-subtle)',
            borderRadius: '8px',
            overflow: 'hidden',
          }}
        >
          <div style={{ padding: '8px' }}>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索技能…"
              style={{
                width: '100%',
                fontSize: '12px',
                padding: '6px 8px',
                border: '1px solid var(--border-subtle)',
                borderRadius: '6px',
                background: 'var(--bg-panel)',
                color: 'var(--text-primary)',
              }}
            />
          </div>
          <div style={{ flex: 1, overflowY: 'auto', padding: '0 6px 8px' }}>
            {loading && (
              <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', padding: '10px' }}>
                正在读取母库…
              </div>
            )}
            {!loading &&
              filtered.map((skill) => {
                const active = selected === skill.name
                return (
                  <button
                    key={skill.name}
                    type="button"
                    onClick={() => void loadTree(skill.name)}
                    style={{
                      display: 'block',
                      width: '100%',
                      textAlign: 'left',
                      padding: '7px 8px',
                      marginBottom: '2px',
                      background: active ? 'var(--bg-element)' : 'transparent',
                      border: 0,
                      borderLeft: active ? '2px solid var(--color-info, #3b82f6)' : '2px solid transparent',
                      borderRadius: '5px',
                      cursor: 'pointer',
                    }}
                  >
                    <div style={{ fontSize: '12px', fontWeight: active ? 500 : 400 }}>
                      {skill.name}
                    </div>
                    <div style={{ fontSize: '10px', color: 'var(--text-tertiary)', marginTop: '2px' }}>
                      {skill.file_count} 文件 · {formatSize(skill.size_bytes)}
                    </div>
                  </button>
                )
              })}
          </div>
        </div>

        <div
          style={{
            width: '210px',
            flex: '0 0 210px',
            display: 'flex',
            flexDirection: 'column',
            minHeight: 0,
            border: '1px solid var(--border-subtle)',
            borderRadius: '8px',
            overflow: 'hidden',
          }}
        >
          <div style={{ padding: '9px 10px', fontSize: '11px', color: 'var(--text-tertiary)' }}>
            {selected ?? '选择一个技能'}
          </div>
          <div style={{ flex: 1, overflowY: 'auto', padding: '0 4px 8px' }}>
            {selected && tree.length === 0 && (
              <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', padding: '6px 8px' }}>
                空目录
              </div>
            )}
            {renderNodes(tree, 0)}
          </div>
        </div>

        <div
          style={{
            flex: 1,
            minWidth: 0,
            display: 'flex',
            flexDirection: 'column',
            border: '1px solid var(--border-subtle)',
            borderRadius: '8px',
            overflow: 'hidden',
          }}
        >
          {!activeFile && (
            <div
              style={{
                flex: 1,
                display: 'grid',
                placeItems: 'center',
                fontSize: '12px',
                color: 'var(--text-tertiary)',
                padding: '20px',
                textAlign: 'center',
              }}
            >
              从左侧选一个文件查看内容
              <br />
              <span style={{ fontSize: '11px' }}>
                这里只读；要改内容请改原件或对应工具里的副本
              </span>
            </div>
          )}
          {activeFile && (
            <>
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: '10px',
                  padding: '8px 12px',
                  borderBottom: '1px solid var(--border-subtle)',
                  fontSize: '11px',
                  color: 'var(--text-tertiary)',
                }}
              >
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {activeFile}
                </span>
                <span>{fileLoading ? '读取中…' : ''}</span>
              </div>
              <div style={{ flex: 1, overflowY: 'auto', padding: '14px 18px' }}>
                {fileError && (
                  <div style={{ fontSize: '12px', color: 'var(--status-error)' }}>{fileError}</div>
                )}
                {!fileError && MARKDOWN_EXT.test(activeFile) && (
                  <>
                    {(() => {
                      const { meta, body } = splitFrontmatter(fileContent)
                      return (
                        <>
                          {meta && (
                            <pre
                              style={{
                                margin: '0 0 14px',
                                padding: '8px 10px',
                                background: 'var(--bg-element)',
                                borderRadius: '6px',
                                fontSize: '11px',
                                lineHeight: 1.6,
                                color: 'var(--text-tertiary)',
                                whiteSpace: 'pre-wrap',
                                wordBreak: 'break-word',
                              }}
                            >
                              {meta}
                            </pre>
                          )}
                          <div className="markdown-body" style={{ fontSize: '13px' }}>
                            <Markdown>{body}</Markdown>
                          </div>
                        </>
                      )
                    })()}
                  </>
                )}
                {!fileError && !MARKDOWN_EXT.test(activeFile) && (
                  <pre
                    style={{
                      margin: 0,
                      fontSize: '12px',
                      lineHeight: 1.6,
                      whiteSpace: 'pre-wrap',
                      wordBreak: 'break-word',
                      color: 'var(--text-secondary)',
                      fontFamily: 'var(--font-mono, monospace)',
                    }}
                  >
                    {fileContent}
                  </pre>
                )}
              </div>
            </>
          )}
        </div>
      </div>

      <div style={{ paddingTop: '10px', fontSize: '10px', color: 'var(--text-tertiary)' }}>
        母库是各工具的镜像汇总，这里是只读视图。当前目录：{current?.path ?? '—'}
      </div>
    </div>
  )
}
