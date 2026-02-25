import { useCallback } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'

/**
 * Returns a mousedown handler that triggers Tauri window drag via startDragging().
 * More reliable than data-tauri-drag-region with titleBarStyle: Overlay in Tauri v2.
 */
export function useDragRegion() {
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest('button, input, select, a, [data-no-drag]')) return
    e.preventDefault()
    getCurrentWindow().startDragging().catch(() => { /* ignore */ })
  }, [])

  return { onMouseDown }
}
