import React, { useEffect, useRef } from 'react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/SkillImprovementSurvey.tsx`.
 *
 * Shown when a skill suggests an improvement to itself. The user picks
 * `1` (apply) or `0` (dismiss) — the digit is read off the bottom of
 * `inputValue` as the user types. Upstream consumes a
 * `FeedbackSurveyResponse` union from `FeedbackSurvey/utils`; here we
 * narrow to `'good' | 'dismissed'` since those are the only two this
 * survey emits.
 */

export type SkillImprovementResponse = 'good' | 'dismissed'

export type SkillUpdate = {
  change: string
}

type Props = {
  isOpen: boolean
  skillName: string
  updates: SkillUpdate[]
  handleSelect: (selected: SkillImprovementResponse) => void
  inputValue: string
  setInputValue: (value: string) => void
}

const BLACK_CIRCLE = '\u25CF'
const BULLET_OPERATOR = '\u2219'
const VALID_INPUTS = ['0', '1'] as const

function isValidInput(input: string): boolean {
  return (VALID_INPUTS as readonly string[]).includes(input)
}

function isValidResponseInput(input: string): boolean {
  if (input.length === 0) return true
  return isValidInput(input)
}

/**
 * Normalize fullwidth digits (U+FF10\u2026U+FF19) to ASCII digits so IME
 * input is still accepted.
 */
function normalizeFullWidthDigits(input: string): string {
  let out = ''
  for (const ch of input) {
    const code = ch.charCodeAt(0)
    if (code >= 0xff10 && code <= 0xff19) {
      out += String.fromCharCode(code - 0xff10 + 0x30)
    } else {
      out += ch
    }
  }
  return out
}

export function SkillImprovementSurvey({
  isOpen,
  skillName,
  updates,
  handleSelect,
  inputValue,
  setInputValue,
}: Props): React.ReactElement | null {
  if (!isOpen) return null
  if (inputValue && !isValidResponseInput(inputValue)) return null

  return (
    <SkillImprovementSurveyView
      skillName={skillName}
      updates={updates}
      onSelect={handleSelect}
      inputValue={inputValue}
      setInputValue={setInputValue}
    />
  )
}

type ViewProps = {
  skillName: string
  updates: SkillUpdate[]
  onSelect: (option: SkillImprovementResponse) => void
  inputValue: string
  setInputValue: (value: string) => void
}

function SkillImprovementSurveyView({
  skillName,
  updates,
  onSelect,
  inputValue,
  setInputValue,
}: ViewProps): React.ReactElement {
  const initialInputValue = useRef(inputValue)

  useEffect(() => {
    if (inputValue !== initialInputValue.current) {
      const lastChar = normalizeFullWidthDigits(inputValue.slice(-1))
      if (isValidInput(lastChar)) {
        setInputValue(inputValue.slice(0, -1))
        onSelect(lastChar === '1' ? 'good' : 'dismissed')
      }
    }
  }, [inputValue, onSelect, setInputValue])

  return (
    <box flexDirection="column" marginTop={1}>
      <box flexDirection="row">
        <text fg={c.info}>{BLACK_CIRCLE} </text>
        <text>
          <strong>Skill improvement suggested for &quot;{skillName}&quot;</strong>
        </text>
      </box>

      <box flexDirection="column" marginLeft={2}>
        {updates.map((u, i) => (
          <text key={i} fg={c.dim}>
            {BULLET_OPERATOR} {u.change}
          </text>
        ))}
      </box>

      <box marginLeft={2} marginTop={1} flexDirection="row">
        <box width={12}>
          <text>
            <span fg={c.info}>1</span>: Apply
          </text>
        </box>
        <box width={14}>
          <text>
            <span fg={c.info}>0</span>: Dismiss
          </text>
        </box>
      </box>
    </box>
  )
}
