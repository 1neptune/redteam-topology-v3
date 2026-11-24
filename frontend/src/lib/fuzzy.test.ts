import { describe, expect, it } from 'vitest'
import { fuzzyMatch, fuzzyScore, bestScore } from './fuzzy'

describe('fuzzyScore', () => {
  it('整串包含时直接命中且分数最低', () => {
    expect(fuzzyScore('Web 应用', 'web')).toBe(0)
  })

  it('大小写不敏感', () => {
    expect(fuzzyScore('Web 应用', 'WEB')).toBe(0)
  })

  it('中文子串命中', () => {
    expect(fuzzyMatch('Web 应用', '应用')).toBe(true)
    expect(fuzzyMatch('入口 / 暴露面', '暴露')).toBe(true)
  })

  it('跨中英文的子序列命中：web应', () => {
    expect(fuzzyMatch('Web 应用', 'web应')).toBe(true)
  })

  it('纯英文 slug 的子序列命中：wbsrv → web-server', () => {
    expect(fuzzyMatch('web-server', 'wbsrv')).toBe(true)
  })

  it('跳跃取字不算命中', () => {
    expect(fuzzyMatch('Web 应用', 'ppaw')).toBe(false)
  })

  it('完全无关返回 null', () => {
    expect(fuzzyScore('数据库', 'zzz')).toBeNull()
  })

  it('空关键字视为全部命中', () => {
    expect(fuzzyScore('数据库', '')).toBe(0)
  })

  it('连续命中比跳跃命中分数低', () => {
    const tight = fuzzyScore('webapp', 'web')
    const loose = fuzzyScore('w-e-b', 'web')
    expect(tight).not.toBeNull()
    expect(loose).not.toBeNull()
    expect(tight!).toBeLessThan(loose!)
  })
})

describe('bestScore', () => {
  it('取候选里的最低分', () => {
    expect(bestScore('web', ['数据库', 'Web 应用', 'web-server'])).toBe(0)
  })

  it('只靠 slug 命中也能算上', () => {
    const score = bestScore('wbsrv', ['Web 应用', 'web-server', '入口'])
    expect(score).not.toBeNull()
  })

  it('全都不命中返回 null', () => {
    expect(bestScore('qqq', ['数据库', 'Web 应用'])).toBeNull()
  })
})