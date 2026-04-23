import React from 'react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'

/**
 * Startup status notices row. The upstream
 * (`ui/examples/upstream-patterns/src/components/StatusNotices.tsx`) pulls
 * `getActiveNotices` from a static registry driven by CLAUDE.md parsing,
 * agent definitions, and trust prompts — none of which the Rust port
 * surfaces yet.
 *
 * This port keeps the render shape (`paddingLeft={1}` column of dimmed
 * lines) but reads from `state.statusNotices` so future backend code can
 * push notices through the normal IPC channel. Individual notice records
 * only need `{ id, text, color? }`; when the list is empty the component
 * renders nothing so it doesn't steal a row in fullscreen layout.
 */

type StatusNoticeRecord = {
  id: string
  text: string
  color?: string
}

export function StatusNotices(): React.ReactElement | null {
  const notices = useAppState(s => {
    const value = (s as unknown as { statusNotices?: StatusNoticeRecord[] })
      .statusNotices
    return value ?? []
  })

  if (notices.length === 0) {
    return null
  }

  return (
    <box flexDirection="column" paddingLeft={1}>
      {notices.map(notice => (
        <text key={notice.id} fg={notice.color ?? c.dim}>
          {notice.text}
        </text>
      ))}
    </box>
  )
}
