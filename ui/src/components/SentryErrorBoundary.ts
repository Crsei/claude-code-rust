import * as React from 'react'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/SentryErrorBoundary.ts`.
 *
 * The Lite frontend doesn't ship the Sentry SDK upstream uses
 * (`src/utils/sentry.ts`). We keep the same contract — a React class
 * error boundary that swallows render errors and logs them — but route
 * the capture to `console.error` so the browser/terminal devtools still
 * surface the failure for debugging.
 */

interface Props {
  children: React.ReactNode
  /** Optional label identifying which component boundary caught the error. */
  name?: string
}

interface State {
  hasError: boolean
}

export class SentryErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false }
  }

  static getDerivedStateFromError(): State {
    return { hasError: true }
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    const boundary = this.props.name || 'SentryErrorBoundary'
    // eslint-disable-next-line no-console
    console.error(`[${boundary}]`, error, errorInfo.componentStack)
  }

  render(): React.ReactNode {
    if (this.state.hasError) {
      return null
    }
    return this.props.children
  }
}
