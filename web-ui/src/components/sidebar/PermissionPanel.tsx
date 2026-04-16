import { useState } from 'react'
import { updateSetting } from '@/lib/api'
import { Shield, Zap, Brain, Gauge } from 'lucide-react'

interface PermissionPanelProps {
  mode: string
  thinking: boolean | null
  fastMode: boolean
  effort: string | null
}

const PERMISSION_MODES = [
  { value: 'default', label: 'Default', desc: 'Ask before risky actions' },
  { value: 'auto', label: 'Auto', desc: 'Auto-approve safe actions' },
  { value: 'bypass', label: 'Bypass', desc: 'Skip all permission checks' },
  { value: 'plan', label: 'Plan', desc: 'Read-only, planning mode' },
]

const EFFORT_LEVELS = [
  { value: 'low', label: 'Low' },
  { value: 'medium', label: 'Medium' },
  { value: 'high', label: 'High' },
]

export function PermissionPanel({ mode, thinking, fastMode, effort }: PermissionPanelProps) {
  const [loading, setLoading] = useState<string | null>(null)

  const handlePermissionChange = async (value: string) => {
    setLoading('permission')
    try {
      await updateSetting('set_permission_mode', value)
    } finally {
      setLoading(null)
    }
  }

  const handleToggle = async (action: string, currentValue: boolean) => {
    setLoading(action)
    try {
      await updateSetting(action, !currentValue)
    } finally {
      setLoading(null)
    }
  }

  const handleEffort = async (value: string) => {
    setLoading('effort')
    try {
      await updateSetting('set_effort', effort === value ? null : value)
    } finally {
      setLoading(null)
    }
  }

  return (
    <div className="space-y-3">
      {/* Permission Mode */}
      <div className="space-y-1.5">
        <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground uppercase tracking-wide">
          <Shield className="h-3 w-3" />
          Permissions
        </div>
        <div className="grid grid-cols-2 gap-1">
          {PERMISSION_MODES.map((pm) => (
            <button
              key={pm.value}
              onClick={() => handlePermissionChange(pm.value)}
              disabled={loading === 'permission'}
              title={pm.desc}
              className={`rounded-md px-2 py-1.5 text-xs transition-colors ${
                mode === pm.value
                  ? 'bg-primary/15 text-primary font-medium'
                  : 'text-foreground/60 hover:bg-muted'
              }`}
            >
              {pm.label}
            </button>
          ))}
        </div>
      </div>

      {/* Toggles */}
      <div className="space-y-1">
        {/* Fast Mode */}
        <button
          onClick={() => handleToggle('set_fast_mode', fastMode)}
          disabled={loading === 'set_fast_mode'}
          className="flex w-full items-center justify-between rounded-md px-2.5 py-1.5 text-xs hover:bg-muted transition-colors"
        >
          <span className="flex items-center gap-2 text-foreground/70">
            <Zap className="h-3 w-3" />
            Fast Mode
          </span>
          <TogglePill active={fastMode} />
        </button>

        {/* Thinking */}
        <button
          onClick={() => handleToggle('set_thinking', thinking === true)}
          disabled={loading === 'set_thinking'}
          className="flex w-full items-center justify-between rounded-md px-2.5 py-1.5 text-xs hover:bg-muted transition-colors"
        >
          <span className="flex items-center gap-2 text-foreground/70">
            <Brain className="h-3 w-3" />
            Thinking
          </span>
          <TogglePill active={thinking === true} />
        </button>
      </div>

      {/* Effort Level */}
      <div className="space-y-1.5">
        <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground uppercase tracking-wide">
          <Gauge className="h-3 w-3" />
          Effort
        </div>
        <div className="flex gap-1">
          {EFFORT_LEVELS.map((level) => (
            <button
              key={level.value}
              onClick={() => handleEffort(level.value)}
              disabled={loading === 'effort'}
              className={`flex-1 rounded-md px-2 py-1.5 text-xs transition-colors ${
                effort === level.value
                  ? 'bg-primary/15 text-primary font-medium'
                  : 'text-foreground/60 hover:bg-muted'
              }`}
            >
              {level.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

function TogglePill({ active }: { active: boolean }) {
  return (
    <div
      className={`relative h-4 w-7 rounded-full transition-colors ${
        active ? 'bg-primary' : 'bg-muted-foreground/30'
      }`}
    >
      <div
        className={`absolute top-0.5 h-3 w-3 rounded-full bg-white transition-transform ${
          active ? 'translate-x-3.5' : 'translate-x-0.5'
        }`}
      />
    </div>
  )
}
