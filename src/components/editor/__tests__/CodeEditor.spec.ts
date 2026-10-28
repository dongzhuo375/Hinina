// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView, settleView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'
import type { EditorPrefs } from '@/services/config.service'

interface Deferred<T> {
  promise: Promise<T>
  resolve: (value: T) => void
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((res) => { resolve = res })
  return { promise, resolve }
}

const mocks = vi.hoisted(() => ({
  getEditorPrefs: vi.fn(),
  updateEditorPrefs: vi.fn(),
  createEditor: vi.fn(),
  setModelLanguage: vi.fn(),
  setTheme: vi.fn(),
}))

vi.mock('monaco-editor', () => ({
  editor: {
    create: mocks.createEditor,
    setModelLanguage: mocks.setModelLanguage,
    setTheme: mocks.setTheme,
  },
}))
vi.mock('monaco-editor/esm/vs/editor/editor.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/typescript/ts.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/css/css.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/html/html.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/json/json.worker?worker', () => ({ default: class {} }))
vi.mock('@/services/config.service', () => ({
  configService: {
    getEditorPrefs: mocks.getEditorPrefs,
    updateEditorPrefs: mocks.updateEditorPrefs,
  },
}))
vi.mock('@/components/editor/EditorSettingsPopover.vue', () => ({
  default: { template: '<div />' },
}))

import CodeEditor from '@/components/editor/CodeEditor.vue'

const PREFS: EditorPrefs = { fontSize: 14, tabSize: 4, editorTheme: 'vs' }

function editorStub() {
  return {
    onDidChangeModelContent: vi.fn(),
    onDidChangeCursorPosition: vi.fn(),
    updateOptions: vi.fn(),
    getValue: vi.fn(() => ''),
    setValue: vi.fn(),
    getModel: vi.fn(() => ({})),
    dispose: vi.fn(),
    focus: vi.fn(),
  }
}

const mounted: Mounted[] = []
async function mount(): Promise<Mounted> {
  const view = await mountView(CodeEditor, {
    props: { modelValue: 'int main() {}', language: 'C++', isDirty: false },
  })
  mounted.push(view)
  return view
}

function unmount(view: Mounted): void {
  const index = mounted.indexOf(view)
  if (index >= 0) mounted.splice(index, 1)
  view.unmount()
}

beforeEach(() => {
  vi.resetAllMocks()
  mocks.getEditorPrefs.mockResolvedValue(PREFS)
  mocks.updateEditorPrefs.mockResolvedValue(undefined)
})

afterEach(() => {
  while (mounted.length > 0) mounted.pop()?.unmount()
  vi.restoreAllMocks()
})

describe('CodeEditor 异步挂载', () => {
  it('偏好读取期间卸载：返回后不创建 Monaco，也不补挂 keydown listener', async () => {
    const prefs = deferred<EditorPrefs>()
    mocks.getEditorPrefs.mockImplementationOnce(() => prefs.promise)
    const addListener = vi.spyOn(window, 'addEventListener')
    const view = await mount()

    unmount(view)
    addListener.mockClear()
    prefs.resolve(PREFS)
    await settleView(12)

    expect(mocks.createEditor).not.toHaveBeenCalled()
    expect(addListener).not.toHaveBeenCalledWith('keydown', expect.any(Function))
  })

  it('正常挂载仍用有效容器创建 Monaco', async () => {
    const editor = editorStub()
    mocks.createEditor.mockReturnValue(editor)
    await mount()

    expect(mocks.createEditor).toHaveBeenCalledTimes(1)
    const [container, options] = mocks.createEditor.mock.calls[0] as [
      HTMLDivElement,
      { value: string },
    ]
    expect(container).toBeInstanceOf(HTMLDivElement)
    expect(options.value).toBe('int main() {}')
    expect(editor.onDidChangeModelContent).toHaveBeenCalledTimes(1)
    expect(editor.onDidChangeCursorPosition).toHaveBeenCalledTimes(1)
  })
})
