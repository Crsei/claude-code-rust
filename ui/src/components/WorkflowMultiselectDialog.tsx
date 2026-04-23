import React, { useCallback, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/WorkflowMultiselectDialog.tsx`.
 *
 * Shown by `/install-github-app` to let the user pick which GitHub
 * Actions workflows to write into the repository. The upstream relies
 * on Ink's `Dialog` + `SelectMulti`; this port draws a bordered frame
 * and implements space-to-toggle / enter-to-submit with `useKeyboard`.
 *
 * The workflow catalog matches upstream one-for-one. Selections default
 * to the caller's `defaultSelections`; submitting with nothing checked
 * surfaces the "must select at least one" error without closing the
 * dialog, so the user can recover without re-opening it.
 */

export type Workflow = 'claude' | 'claude-review'

type WorkflowOption = {
  value: Workflow
  label: string
}

const WORKFLOWS: WorkflowOption[] = [
  {
    value: 'claude',
    label: '@Claude Code - Tag @claude in issues and PR comments',
  },
  {
    value: 'claude-review',
    label: 'Claude Code Review - Automated code review on new PRs',
  },
]

const EXAMPLES_URL =
  'https://placeholder.invalid/github.com/anthropics/claude-code-action/blob/main/examples/'

type Props = {
  onSubmit: (selectedWorkflows: Workflow[]) => void
  defaultSelections: Workflow[]
}

export function WorkflowMultiselectDialog({
  onSubmit,
  defaultSelections,
}: Props): React.ReactElement {
  const [selected, setSelected] = useState<Set<Workflow>>(
    () => new Set(defaultSelections),
  )
  const [focusIndex, setFocusIndex] = useState(0)
  const [showError, setShowError] = useState(false)

  const handleSubmit = useCallback(() => {
    const values = WORKFLOWS.filter(w => selected.has(w.value)).map(w => w.value)
    if (values.length === 0) {
      setShowError(true)
      return
    }
    setShowError(false)
    onSubmit(values)
  }, [onSubmit, selected])

  const toggleAt = useCallback((index: number) => {
    const workflow = WORKFLOWS[index]
    if (!workflow) return
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(workflow.value)) {
        next.delete(workflow.value)
      } else {
        next.add(workflow.value)
      }
      return next
    })
    setShowError(false)
  }, [])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence

    if (name === 'up') {
      setFocusIndex(i => (i - 1 + WORKFLOWS.length) % WORKFLOWS.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      setFocusIndex(i => (i + 1) % WORKFLOWS.length)
      return
    }
    if (name === 'space' || seq === ' ') {
      toggleAt(focusIndex)
      return
    }
    if (name === 'return' || name === 'enter') {
      handleSubmit()
      return
    }
    if (name === 'escape') {
      setShowError(true)
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title="Select GitHub workflows to install"
      titleAlignment="center"
    >
      <text fg={c.dim}>
        We'll create a workflow file in your repository for each one you select.
      </text>

      <box marginTop={1}>
        <text fg={c.dim}>
          More workflow examples (issue triage, CI fixes, etc.) at: {EXAMPLES_URL}
        </text>
      </box>

      <box marginTop={1} flexDirection="column">
        {WORKFLOWS.map((workflow, i) => {
          const isFocused = i === focusIndex
          const isChecked = selected.has(workflow.value)
          const marker = isChecked ? '[x]' : '[ ]'
          return (
            <box key={workflow.value} flexDirection="row">
              <text fg={isFocused ? c.accent : c.text}>
                {isFocused ? '\u25B8 ' : '  '}
              </text>
              <text fg={isChecked ? c.success : c.dim}>{marker} </text>
              <text fg={isFocused ? c.textBright : c.text}>
                {workflow.label}
              </text>
            </box>
          )
        })}
      </box>

      {showError && (
        <box marginTop={1}>
          <text fg={c.error}>
            You must select at least one workflow to continue
          </text>
        </box>
      )}

      <box marginTop={1}>
        <text fg={c.dim}>
          <em>{'\u2191\u2193 navigate \u00b7 Space toggle \u00b7 Enter confirm \u00b7 Esc cancel'}</em>
        </text>
      </box>
    </box>
  )
}
