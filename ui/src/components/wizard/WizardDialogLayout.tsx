import React, { type ReactNode } from 'react'
import { c } from '../../theme.js'
import { useWizard } from './useWizard.js'
import { WizardNavigationFooter } from './WizardNavigationFooter.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/wizard/WizardDialogLayout.tsx`.
 *
 * Wraps a wizard step with a bordered frame and the navigation footer.
 * Upstream uses Ink's `Dialog` primitive; this port draws the frame
 * directly using OpenTUI intrinsics so it doesn't depend on the
 * design-system bundle.
 */

type Props = {
  /** Override the title pulled from `useWizard().title`. */
  title?: string
  /** Border / title colour (defaults to the accent). */
  color?: string
  children: ReactNode
  subtitle?: string
  footerText?: ReactNode
  pendingExitText?: string
}

export function WizardDialogLayout({
  title: titleOverride,
  color = c.accent,
  children,
  subtitle,
  footerText,
  pendingExitText,
}: Props): React.ReactElement {
  const {
    currentStepIndex,
    totalSteps,
    title: providerTitle,
    showStepCounter,
  } = useWizard()
  const title = titleOverride || providerTitle || 'Wizard'
  const stepSuffix =
    showStepCounter !== false ? ` (${currentStepIndex + 1}/${totalSteps})` : ''

  return (
    <>
      <box
        flexDirection="column"
        borderStyle="rounded"
        borderColor={color}
        paddingX={2}
        paddingY={1}
        title={`${title}${stepSuffix}`}
        titleAlignment="center"
      >
        {subtitle ? (
          <box marginBottom={1}>
            <text fg={c.dim}>{subtitle}</text>
          </box>
        ) : null}
        {children}
      </box>
      <WizardNavigationFooter
        instructions={footerText}
        pendingExitText={pendingExitText}
      />
    </>
  )
}
