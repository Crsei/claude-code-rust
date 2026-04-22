import { RGBA } from '@opentui/core'
import type { FrontendMessage } from '../ipc/protocol.js'

type ResizeBackend = {
  send(message: FrontendMessage): void
}

type ClearableBuffer = {
  clear(color: RGBA): void
}

type ResizeRenderer = {
  currentRenderBuffer: ClearableBuffer
  nextRenderBuffer: ClearableBuffer
  intermediateRender(): void
  setBackgroundColor(color: RGBA): void
  rendererPtr?: unknown
  lib?: {
    clearTerminal?(rendererPtr: unknown): void
  }
}

const TERMINAL_BG = RGBA.fromInts(0, 0, 0, 255)

export function notifyBackendResize(
  backend: ResizeBackend,
  width: number,
  height: number,
): void {
  backend.send({ type: 'resize', cols: width, rows: height })
}

export function clearTerminalBuffers(renderer: ResizeRenderer): void {
  clearBuffer(renderer.currentRenderBuffer)
  clearBuffer(renderer.nextRenderBuffer)
}

export function ensureOpaqueRendererBackground(renderer: ResizeRenderer): void {
  renderer.setBackgroundColor(TERMINAL_BG)
}

export function repaintAfterResize(renderer: ResizeRenderer): void {
  ensureOpaqueRendererBackground(renderer)
  clearTerminalSurface(renderer)
  clearTerminalBuffers(renderer)
  renderer.intermediateRender()
}

function clearBuffer(buffer: ClearableBuffer): void {
  buffer.clear(TERMINAL_BG)
}

function clearTerminalSurface(renderer: ResizeRenderer): void {
  if (!renderer.rendererPtr) {
    return
  }

  renderer.lib?.clearTerminal?.(renderer.rendererPtr)
}
