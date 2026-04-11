import React, { useEffect, useState } from 'react'

const FRAMES = ['\u280B', '\u2819', '\u2839', '\u2838', '\u283C', '\u2834', '\u2826', '\u2827', '\u2807', '\u280F']

export function Spinner({ label = 'Thinking...' }: { label?: string }) {
  const [frame, setFrame] = useState(0)

  useEffect(() => {
    const id = setInterval(() => {
      setFrame(f => (f + 1) % FRAMES.length)
    }, 80)
    return () => clearInterval(id)
  }, [])

  return (
    <box paddingX={1}>
      <text fg="#00AAAA">{FRAMES[frame]} {label}</text>
    </box>
  )
}
