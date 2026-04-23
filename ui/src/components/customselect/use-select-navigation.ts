import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import OptionMap from './option-map.js'
import type { OptionWithDescription } from './select.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/use-select-navigation.ts`.
 *
 * Upstream layers paging, wrap-around, input-mode hopping, and disabled
 * skipping on top of the linked-list node map. The Lite port keeps the
 * same public contract (focus navigation + `visibleFromIndex` /
 * `visibleToIndex` window) and enough behaviour for the pickers that
 * ship in Lite — wrap-around, per-page moves, disabled skipping,
 * direct `focusOption(value)`. Upstream's input-mode coupling is
 * covered by the separate `useSelectInput` hook.
 */

export type UseSelectNavigationProps<T> = {
  visibleOptionCount?: number
  options: OptionWithDescription<T>[]
  initialFocusValue?: T
  focusValue?: T
  onFocus?: (value: T) => void
}

export type NavigationState<T> = {
  focusedValue: T | undefined
  focusedIndex: number
  visibleFromIndex: number
  visibleToIndex: number
  options: OptionWithDescription<T>[]
  visibleOptions: Array<OptionWithDescription<T> & { index: number }>
  isInInput: boolean
  focusNextOption: () => void
  focusPreviousOption: () => void
  focusNextPage: () => void
  focusPreviousPage: () => void
  focusOption: (value: T | undefined) => void
}

function isDisabled<T>(opt: OptionWithDescription<T> | undefined): boolean {
  return !!opt && 'disabled' in opt && opt.disabled === true
}

function findNextEnabled<T>(
  options: OptionWithDescription<T>[],
  startIndex: number,
  step: 1 | -1,
): number {
  const len = options.length
  if (len === 0) return -1
  let idx = startIndex
  for (let i = 0; i < len; i++) {
    if (!isDisabled(options[idx])) return idx
    idx += step
    if (idx < 0) idx = len - 1
    if (idx >= len) idx = 0
  }
  return -1
}

export function useSelectNavigation<T>({
  visibleOptionCount = 5,
  options,
  initialFocusValue,
  focusValue,
  onFocus,
}: UseSelectNavigationProps<T>): NavigationState<T> {
  const map = useMemo(() => new OptionMap<T>(options), [options])
  const clampCount = Math.min(Math.max(1, visibleOptionCount), options.length || 1)

  const initialIndex = useMemo(() => {
    if (initialFocusValue === undefined) {
      return findNextEnabled(options, 0, 1)
    }
    const hit = map.get(initialFocusValue)?.index ?? -1
    if (hit < 0) return findNextEnabled(options, 0, 1)
    return isDisabled(options[hit]) ? findNextEnabled(options, hit, 1) : hit
  }, [map, options, initialFocusValue])

  const [focusedIndex, setFocusedIndex] = useState<number>(initialIndex)
  const [visibleFromIndex, setVisibleFromIndex] = useState<number>(
    Math.max(0, Math.min(initialIndex, Math.max(0, options.length - clampCount))),
  )

  const lastNotifiedRef = useRef<T | undefined>(undefined)

  useEffect(() => {
    if (focusValue === undefined) return
    const hit = map.get(focusValue)?.index
    if (hit === undefined || hit === focusedIndex) return
    setFocusedIndex(hit)
    setVisibleFromIndex(prev => clampWindow(prev, hit, clampCount, options.length))
  }, [focusValue, map, focusedIndex, clampCount, options.length])

  useEffect(() => {
    if (focusedIndex < 0) return
    const v = options[focusedIndex]?.value as T | undefined
    if (v === undefined) return
    if (lastNotifiedRef.current === v) return
    lastNotifiedRef.current = v
    onFocus?.(v)
  }, [focusedIndex, options, onFocus])

  const visibleToIndex = Math.min(
    options.length,
    visibleFromIndex + clampCount,
  )

  const visibleOptions = useMemo(() => {
    return options.slice(visibleFromIndex, visibleToIndex).map((opt, i) => ({
      ...opt,
      index: visibleFromIndex + i,
    }))
  }, [options, visibleFromIndex, visibleToIndex])

  const focusedValue =
    focusedIndex >= 0 ? (options[focusedIndex]?.value as T | undefined) : undefined

  const isInInput = useMemo(() => {
    const opt = options[focusedIndex]
    return !!opt && 'type' in opt && opt.type === 'input'
  }, [options, focusedIndex])

  const focusNextOption = useCallback(() => {
    setFocusedIndex(prev => {
      const len = options.length
      if (len === 0) return -1
      let next = prev + 1
      if (next >= len) next = 0
      next = findNextEnabled(options, next, 1)
      if (next < 0) return prev
      setVisibleFromIndex(v => clampWindow(v, next, clampCount, len))
      return next
    })
  }, [options, clampCount])

  const focusPreviousOption = useCallback(() => {
    setFocusedIndex(prev => {
      const len = options.length
      if (len === 0) return -1
      let next = prev - 1
      if (next < 0) next = len - 1
      next = findNextEnabled(options, next, -1)
      if (next < 0) return prev
      setVisibleFromIndex(v => clampWindow(v, next, clampCount, len))
      return next
    })
  }, [options, clampCount])

  const focusNextPage = useCallback(() => {
    setFocusedIndex(prev => {
      const len = options.length
      if (len === 0) return -1
      let next = Math.min(len - 1, prev + clampCount)
      next = findNextEnabled(options, next, -1)
      if (next < 0) return prev
      setVisibleFromIndex(v => clampWindow(v, next, clampCount, len))
      return next
    })
  }, [options, clampCount])

  const focusPreviousPage = useCallback(() => {
    setFocusedIndex(prev => {
      const len = options.length
      if (len === 0) return -1
      let next = Math.max(0, prev - clampCount)
      next = findNextEnabled(options, next, 1)
      if (next < 0) return prev
      setVisibleFromIndex(v => clampWindow(v, next, clampCount, len))
      return next
    })
  }, [options, clampCount])

  const focusOption = useCallback(
    (value: T | undefined) => {
      if (value === undefined) return
      const hit = map.get(value)?.index
      if (hit === undefined) return
      setFocusedIndex(hit)
      setVisibleFromIndex(v => clampWindow(v, hit, clampCount, options.length))
    },
    [map, clampCount, options.length],
  )

  return {
    focusedValue,
    focusedIndex: focusedIndex >= 0 ? focusedIndex + 1 : 0,
    visibleFromIndex,
    visibleToIndex,
    options,
    visibleOptions,
    isInInput,
    focusNextOption,
    focusPreviousOption,
    focusNextPage,
    focusPreviousPage,
    focusOption,
  }
}

function clampWindow(
  currentFrom: number,
  focusedIdx: number,
  count: number,
  total: number,
): number {
  if (total <= count) return 0
  const max = total - count
  if (focusedIdx < currentFrom) return Math.max(0, focusedIdx)
  if (focusedIdx >= currentFrom + count) {
    return Math.min(max, focusedIdx - count + 1)
  }
  return Math.min(max, currentFrom)
}
