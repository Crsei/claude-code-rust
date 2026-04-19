/** Update an item in an array by key, or append a stub if missing. */
export function upsertBy<T extends Record<string, any>>(
  arr: T[],
  key: keyof T,
  value: any,
  updater: (existing: T) => T,
): T[] {
  const idx = arr.findIndex(item => item[key] === value)
  if (idx >= 0) {
    const copy = [...arr]
    copy[idx] = updater(copy[idx]!)
    return copy
  }
  return [...arr, updater({ [key]: value } as T)]
}
