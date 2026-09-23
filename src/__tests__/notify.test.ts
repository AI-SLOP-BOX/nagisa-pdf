import { describe, it, expect } from 'vitest'
import {
  notifyAppToast,
  notifyError,
  notifyWarning,
  notifySuccess,
  notifyInfo,
  type AppToastDetail,
} from '../utils/notify'

/** app-toast イベントを記録するヘルパー。 */
function captureToasts() {
  const events: AppToastDetail[] = []
  const handler = (e: Event) => events.push((e as CustomEvent<AppToastDetail>).detail)
  window.addEventListener('app-toast', handler)
  return { events, off: () => window.removeEventListener('app-toast', handler) }
}

describe('notify', () => {
  it('notifyAppToast は message/type 付きで app-toast を発火する', () => {
    const { events, off } = captureToasts()
    notifyAppToast('こんにちは', 'success')
    off()
    expect(events).toHaveLength(1)
    expect(events[0].message).toBe('こんにちは')
    expect(events[0].type).toBe('success')
  })

  it('デフォルト type は info', () => {
    const { events, off } = captureToasts()
    notifyAppToast('a')
    off()
    expect(events[0].type).toBe('info')
  })

  it('error 詳細を任意で添えられる', () => {
    const { events, off } = captureToasts()
    notifyAppToast('失敗', 'error', 'boom')
    off()
    expect(events[0].error).toBe('boom')
  })

  it('各ヘルパーが正しい type を設定する', () => {
    const { events, off } = captureToasts()
    notifyError('e')
    notifyWarning('w')
    notifySuccess('s')
    notifyInfo('i')
    off()
    expect(events.map(e => e.type)).toEqual(['error', 'warning', 'success', 'info'])
  })
})
