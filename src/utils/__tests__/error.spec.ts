import { describe, expect, it } from 'vitest'
import { errorMessage } from '@/utils/error'

describe('errorMessage — 错误文案收敛', () => {
  it('Error / IpcError 取 message', () => {
    expect(errorMessage(new Error('磁盘写入失败'), '保存配置失败')).toBe('磁盘写入失败')
    expect(errorMessage(new Error('登录失败: 用户名或密码错误'), '登录失败')).toBe(
      '登录失败: 用户名或密码错误',
    )
  })

  it('字符串载荷原样返回（trim 后）', () => {
    expect(errorMessage('  网络不可达  ', '提交失败')).toBe('网络不可达')
  })

  it('无信息载荷回退兜底文案（空 message / 非 Error 对象 / null / undefined）', () => {
    expect(errorMessage(new Error(''), '加载题目失败')).toBe('加载题目失败')
    expect(errorMessage(new Error('   '), '加载题目失败')).toBe('加载题目失败')
    expect(errorMessage('', '加载题目失败')).toBe('加载题目失败')
    expect(errorMessage({ code: 500 }, '加载题目失败')).toBe('加载题目失败')
    expect(errorMessage(null, '加载题目失败')).toBe('加载题目失败')
    expect(errorMessage(undefined, '加载题目失败')).toBe('加载题目失败')
  })
})
