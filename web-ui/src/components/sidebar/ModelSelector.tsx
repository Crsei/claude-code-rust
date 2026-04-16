import { useState } from 'react'
import { updateSetting } from '@/lib/api'
import { Cpu, Check } from 'lucide-react'

interface ModelSelectorProps {
  model: string
}

const MODEL_PRESETS = [
  { label: 'DeepSeek Reasoner', value: 'deepseek-reasoner' },
  { label: 'DeepSeek Chat', value: 'deepseek-chat' },
  { label: 'Claude Sonnet 4', value: 'claude-sonnet-4-20250514' },
  { label: 'Claude Opus 4', value: 'claude-opus-4-20250514' },
  { label: 'Claude Haiku 3.5', value: 'claude-haiku-3-5-20241022' },
  { label: 'GPT-4o', value: 'gpt-4o' },
  { label: 'GPT-4.1', value: 'gpt-4.1' },
  { label: 'Gemini 2.5 Pro', value: 'gemini-2.5-pro-preview-05-06' },
  { label: 'o3', value: 'o3' },
  { label: 'o4-mini', value: 'o4-mini' },
]

export function ModelSelector({ model }: ModelSelectorProps) {
  const [customModel, setCustomModel] = useState('')
  const [loading, setLoading] = useState(false)
  const [showCustom, setShowCustom] = useState(false)

  const handleSelect = async (value: string) => {
    setLoading(true)
    try {
      await updateSetting('set_model', value)
    } finally {
      setLoading(false)
    }
  }

  const handleCustomSubmit = async () => {
    if (!customModel.trim()) return
    await handleSelect(customModel.trim())
    setCustomModel('')
    setShowCustom(false)
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground uppercase tracking-wide">
        <Cpu className="h-3 w-3" />
        Model
      </div>

      <div className="space-y-0.5">
        {MODEL_PRESETS.map((preset) => (
          <button
            key={preset.value}
            onClick={() => handleSelect(preset.value)}
            disabled={loading}
            className={`flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-xs transition-colors ${
              model === preset.value
                ? 'bg-primary/15 text-primary'
                : 'text-foreground/70 hover:bg-muted'
            }`}
          >
            {model === preset.value && <Check className="h-3 w-3 shrink-0" />}
            <span className={model === preset.value ? '' : 'ml-5'}>{preset.label}</span>
            {model === preset.value && (
              <span className="ml-auto text-[10px] text-primary/60">active</span>
            )}
          </button>
        ))}
      </div>

      {/* Custom model input */}
      {showCustom ? (
        <div className="flex gap-1">
          <input
            type="text"
            value={customModel}
            onChange={(e) => setCustomModel(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleCustomSubmit()}
            placeholder="model-name"
            className="flex-1 rounded border border-border bg-muted/50 px-2 py-1 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            autoFocus
          />
          <button
            onClick={handleCustomSubmit}
            className="rounded bg-primary px-2 py-1 text-xs text-primary-foreground hover:bg-primary/80"
          >
            Set
          </button>
        </div>
      ) : (
        <button
          onClick={() => setShowCustom(true)}
          className="w-full rounded-md border border-dashed border-border px-2.5 py-1.5 text-xs text-muted-foreground hover:text-foreground hover:border-foreground/30 transition-colors"
        >
          + Custom model...
        </button>
      )}

      {/* Current model display if not in presets */}
      {!MODEL_PRESETS.some(p => p.value === model) && (
        <div className="rounded bg-muted/50 px-2.5 py-1.5 text-[11px] font-mono text-foreground/60">
          Current: {model}
        </div>
      )}
    </div>
  )
}
