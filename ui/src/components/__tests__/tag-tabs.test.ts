import { describe, expect, test } from 'bun:test'
import { planTagTabs } from '../TagTabs.js'

describe('planTagTabs', () => {
  test('shows every tab when they fit in the available width', () => {
    const plan = planTagTabs({
      tabs: ['All', 'alpha', 'beta'],
      selectedIndex: 1,
      availableWidth: 200,
      resumeLabelWidth: 7,
    })
    expect(plan.startIndex).toBe(0)
    expect(plan.endIndex).toBe(3)
    expect(plan.hiddenLeft).toBe(0)
    expect(plan.hiddenRight).toBe(0)
    expect(plan.selectedIndex).toBe(1)
  })

  test('collapses into a window centered on the selected tab when space is tight', () => {
    const tabs = Array.from({ length: 20 }, (_, i) => `tag${i}`)
    const plan = planTagTabs({
      tabs,
      selectedIndex: 10,
      availableWidth: 50,
      resumeLabelWidth: 7,
    })
    expect(plan.startIndex).toBeLessThanOrEqual(10)
    expect(plan.endIndex).toBeGreaterThan(10)
    // Either side should hide at least one tab in a 20-tab list at width 50.
    expect(plan.hiddenLeft + plan.hiddenRight).toBeGreaterThan(0)
    expect(plan.selectedIndex).toBe(10)
  })

  test('clamps the selected index to the last tab', () => {
    const plan = planTagTabs({
      tabs: ['All', 'alpha'],
      selectedIndex: 99,
      availableWidth: 200,
      resumeLabelWidth: 7,
    })
    expect(plan.selectedIndex).toBe(1)
  })

  test('handles a single tab without error', () => {
    const plan = planTagTabs({
      tabs: ['All'],
      selectedIndex: 0,
      availableWidth: 200,
      resumeLabelWidth: 7,
    })
    expect(plan.startIndex).toBe(0)
    expect(plan.endIndex).toBe(1)
    expect(plan.hiddenLeft).toBe(0)
    expect(plan.hiddenRight).toBe(0)
  })
})
