import { Component, type ReactNode, StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import './i18n'
import App from './App.tsx'

class GlobalErrorBoundary extends Component<
  { children: ReactNode },
  { hasError: boolean; error: Error | null }
> {
  constructor(props: { children: ReactNode }) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Fatal Frontend Crash:', error, errorInfo)
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            padding: 32,
            color: '#f87171',
            background: '#0f172a',
            height: '100vh',
            fontFamily: 'monospace',
            overflow: 'auto',
            boxSizing: 'border-box',
          }}
        >
          <h1 style={{ fontSize: 20, marginBottom: 16 }}>
            ⚠️ 前端渲染遇到异常（UI Crash）
          </h1>
          <p style={{ color: '#94a3b8', fontSize: 13, marginBottom: 12 }}>
            若持续出现此提示，可按 F12 打开开发者工具查看详细日志，或点击下方按钮重新加载。
          </p>
          <pre
            style={{
              whiteSpace: 'pre-wrap',
              background: '#1e293b',
              padding: 16,
              borderRadius: 8,
              border: '1px solid #334155',
              fontSize: 12,
              lineHeight: 1.5,
            }}
          >
            {this.state.error?.stack || this.state.error?.message}
          </pre>
          <button
            type="button"
            onClick={() => window.location.reload()}
            style={{
              marginTop: 16,
              padding: '8px 18px',
              background: '#3b82f6',
              color: '#fff',
              border: 'none',
              borderRadius: 6,
              cursor: 'pointer',
              fontWeight: 600,
            }}
          >
            重新加载界面
          </button>
        </div>
      )
    }
    return this.props.children
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <GlobalErrorBoundary>
      <App />
    </GlobalErrorBoundary>
  </StrictMode>,
)



