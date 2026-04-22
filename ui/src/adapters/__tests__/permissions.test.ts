import { describe, expect, test } from 'bun:test'
import {
  categorizePermissionTool,
  mapPermissionRequestToViewModel,
  parsePermissionOption,
} from '../permissions.js'

describe('parsePermissionOption', () => {
  test('extracts a trailing hotkey', () => {
    expect(parsePermissionOption('Yes (y)')).toEqual({
      value: 'Yes (y)',
      label: 'Yes',
      hotkey: 'y',
    })
  })

  test('lowercases uppercase hotkeys and keeps the original value', () => {
    expect(parsePermissionOption('Always (A)')).toEqual({
      value: 'Always (A)',
      label: 'Always',
      hotkey: 'a',
    })
  })

  test('falls back to the raw label when no hotkey is present', () => {
    expect(parsePermissionOption('Approve once')).toEqual({
      value: 'Approve once',
      label: 'Approve once',
    })
  })
})

describe('categorizePermissionTool', () => {
  test('groups shell tools', () => {
    expect(categorizePermissionTool('Bash')).toBe('bash')
    expect(categorizePermissionTool('PowerShell')).toBe('bash')
  })

  test('groups file edit tools', () => {
    expect(categorizePermissionTool('Edit')).toBe('file_edit')
    expect(categorizePermissionTool('MultiEdit')).toBe('file_edit')
    expect(categorizePermissionTool('NotebookEdit')).toBe('file_edit')
  })

  test('groups file write tools', () => {
    expect(categorizePermissionTool('Write')).toBe('file_write')
  })

  test('groups fetch/search tools', () => {
    expect(categorizePermissionTool('WebFetch')).toBe('web_fetch')
    expect(categorizePermissionTool('WebSearch')).toBe('web_fetch')
  })

  test('falls back to tool_generic for unknown tools', () => {
    expect(categorizePermissionTool('SomeCustomTool')).toBe('tool_generic')
  })
})

describe('mapPermissionRequestToViewModel', () => {
  test('normalizes the permission request options and category', () => {
    const vm = mapPermissionRequestToViewModel({
      toolUseId: 'pr-1',
      tool: 'Bash',
      command: 'ls -la',
      options: ['Yes (y)', 'No (n)', 'Always allow for session (a)'],
    })

    expect(vm.kind).toBe('permission_request')
    expect(vm.toolUseId).toBe('pr-1')
    expect(vm.tool).toBe('Bash')
    expect(vm.category).toBe('bash')
    expect(vm.options).toEqual([
      { value: 'Yes (y)', label: 'Yes', hotkey: 'y' },
      { value: 'No (n)', label: 'No', hotkey: 'n' },
      {
        value: 'Always allow for session (a)',
        label: 'Always allow for session',
        hotkey: 'a',
      },
    ])
  })
})
