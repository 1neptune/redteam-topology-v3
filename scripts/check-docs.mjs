#!/usr/bin/env node
/**
 * 文档完整性 / 一致性检查。
 *
 * 每次改动接口、工具或版本号后跑一遍：
 *   node scripts/check-docs.mjs
 *
 * 退出码 0 表示全部通过，1 表示有不一致（用于 CI 或提交前钩子）。
 */
import { readFileSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const read = (rel) => readFileSync(join(ROOT, rel), 'utf8')
const has = (rel) => existsSync(join(ROOT, rel))

const fails = []
const warns = []
const pass = []

const fail = (msg) => fails.push(msg)
const warn = (msg) => warns.push(msg)
const ok = (msg) => pass.push(msg)

// ── 1. 版本号六处一致 ─────────────────────────────────────
function versionOf(rel, pattern) {
  if (!has(rel)) return null
  const m = read(rel).match(pattern)
  return m ? m[1] : null
}

const versionSources = [
  ['backend/Cargo.toml', /^version\s*=\s*"([^"]+)"/m],
  ['frontend/package.json', /"version"\s*:\s*"([^"]+)"/],
  ['docker-compose.yml', /image:\s*redteam-atlas:([0-9.]+)/],
  ['Dockerfile', /org\.opencontainers\.image\.version="([^"]+)"/],
  ['doc/openapi.md', /^\s{2}version:\s*([0-9.]+)/m],
  ['doc/mcp.md', /"version":\s*"([0-9.]+)"/],
]

const versions = new Map()
for (const [rel, pattern] of versionSources) {
  const v = versionOf(rel, pattern)
  if (!v) fail(`版本号缺失：${rel} 里找不到版本声明`)
  else versions.set(rel, v)
}

const unique = [...new Set(versions.values())]
if (unique.length === 1) ok(`版本号六处一致：${unique[0]}`)
else if (unique.length > 1) {
  fail(`版本号不一致：${[...versions].map(([f, v]) => `${f}=${v}`).join(', ')}`)
}

const currentVersion = unique[0] ?? ''

// ── 2. CHANGELOG 顶部版本 = 代码版本 ───────────────────────
if (has('CHANGELOG.md')) {
  const top = read('CHANGELOG.md').match(/^## v([0-9.]+)/m)
  if (!top) fail('CHANGELOG.md 里找不到版本小节')
  else if (top[1] !== currentVersion) {
    fail(`CHANGELOG 顶部是 v${top[1]}，代码是 ${currentVersion}——发版时两处必须同步`)
  } else ok(`CHANGELOG 顶部版本与代码一致：v${top[1]}`)
}

// ── 3. openapi 的路由 vs 源码路由 ─────────────────────────
/** 路径参数名不影响对外契约，比较时统一成 {} */
function normalizePath(path) {
  return path.replace(/\{[^}]+\}/g, '{}')
}

function sourceRoutes() {
  const src = read('backend/src/router/mod.rs')
  const out = new Set()
  // 按括号平衡截取每个 .route( ... ) 的完整块，兼容跨行的链式 get().put()
  for (let i = 0; i < src.length; i++) {
    if (!src.startsWith('.route(', i)) continue
    let depth = 0
    let end = i + 6
    for (; end < src.length; end++) {
      if (src[end] === '(') depth++
      else if (src[end] === ')') {
        depth--
        if (depth === 0) break
      }
    }
    const body = src.slice(i + 7, end)
    const pathMatch = body.match(/^\s*"([^"]+)"/)
    if (!pathMatch) continue
    const path = pathMatch[1]
    for (const method of ['get', 'post', 'put', 'patch', 'delete']) {
      if (new RegExp(`\\b${method}\\(`).test(body)) {
        out.add(`${method.toUpperCase()} ${normalizePath(path)}`)
      }
    }
  }
  return out
}

function docRoutes() {
  const doc = read('doc/openapi.md')
  const lines = doc.split('\n')
  const out = new Set()
  let path = ''
  let inYaml = false
  for (const line of lines) {
    if (/^```ya?ml/.test(line)) { inYaml = true; continue }
    if (/^```$/.test(line)) { inYaml = false; continue }
    if (!inYaml) continue
    const p = line.match(/^ {2}(\/[a-zA-Z0-9{}/_-]*):/)
    if (p) { path = p[1]; continue }
    const m = line.match(/^ {4}(get|post|put|patch|delete):/)
    if (m && path) out.add(`${m[1].toUpperCase()} ${normalizePath(path)}`)
  }
  return out
}

if (has('backend/src/router/mod.rs') && has('doc/openapi.md')) {
  const src = sourceRoutes()
  const doc = docRoutes()
  // MCP 是 JSON-RPC，不进 openapi（doc/mcp.md 单独成节）
  const skip = (r) => r.endsWith(' /mcp')
  const missInDoc = [...src].filter((r) => !skip(r) && !doc.has(r))
  const missInSrc = [...doc].filter((r) => !src.has(r))
  if (missInDoc.length) fail(`openapi 缺接口：${missInDoc.join(', ')}`)
  if (missInSrc.length) fail(`openapi 多出源码没有的接口：${missInSrc.join(', ')}`)
  if (!missInDoc.length && !missInSrc.length) {
    const real = [...src].filter((r) => !skip(r))
    ok(`openapi 与源码路由一致：${real.length} 个方法 / ${new Set(real.map((r) => r.split(' ')[1])).size} 个路径`)
  }
}

// ── 4. mcp.md 的工具 vs 源码工具 ──────────────────────────
if (has('backend/src/mcp/tools.rs') && has('doc/mcp.md')) {
  const src = new Set([...read('backend/src/mcp/tools.rs').matchAll(/"name":\s*"([a-z_]+)"/g)].map((m) => m[1]))
  const doc = new Set([...read('doc/mcp.md').matchAll(/^### 4\.\d+\s+([a-z_]+)/gm)].map((m) => m[1]))
  const missInDoc = [...src].filter((t) => !doc.has(t))
  const missInSrc = [...doc].filter((t) => !src.has(t))
  if (missInDoc.length) fail(`mcp.md 缺工具：${missInDoc.join(', ')}`)
  if (missInSrc.length) fail(`mcp.md 多出源码没有的工具：${missInSrc.join(', ')}`)
  if (!missInDoc.length && !missInSrc.length) ok(`mcp.md 工具与源码一致：${src.size} 个`)
}

// ── 5. srs 功能点编号重复与空号 ───────────────────────────
if (has('doc/srs.md')) {
  const feats = [...read('doc/srs.md').matchAll(/^- (F-[0-9a-z]+)\s/gm)].map((m) => m[1])
  const dup = feats.filter((f, i) => feats.indexOf(f) !== i)
  if (dup.length) fail(`srs 功能点编号重复：${[...new Set(dup)].join(', ')}`)
  else ok(`srs 功能点 ${feats.length} 个，无重复编号`)
}

// ── 6. 已移除功能不得再出现在源码与现行文档里 ──────────────
// CHANGELOG 是历史记录，允许出现这些词，单独排除
const removedConcepts = [
  [/^doc\//, /update\.json|updateAvailable|fetchLatestRelease|检查更新|一键更新/],
  [/^backend\//, /update_file|ATLAS_UPDATE_FILE|upgrade_service/],
  [/^frontend\//, /fetchLatestRelease|runUpgrade|isNewerVersion|UpdateDialog/],
]
const sourceFiles = [
  'backend/src/config.rs',
  'backend/src/main.rs',
  'backend/src/handler/system_handler.rs',
  'frontend/src/api/system.ts',
  'frontend/src/types/system.ts',
]
for (const rel of sourceFiles) {
  if (!has(rel)) continue
  const text = read(rel)
  for (const [scope, re] of removedConcepts) {
    if (!scope.test(rel)) continue
    if (re.test(text)) fail(`${rel} 仍含已移除的概念：${re.source}`)
  }
}
if (!fails.some((f) => f.includes('已移除的概念'))) ok('源码里没有已移除功能的残留')

// ── 7. 文档引用的文件是否存在 ─────────────────────────────
if (has('README.md')) {
  const refs = [...read('README.md').matchAll(/!\[[^\]]*\]\(([^)]+)\)/g)].map((m) => m[1])
  const missing = refs.filter((r) => !/^https?:/.test(r) && !has(r))
  if (missing.length) fail(`README 引用的图片不存在：${missing.join(', ')}`)
  else if (refs.length) ok(`README 图片引用 ${refs.length} 个，全部存在`)
}

// ── 8. 迁移文件与 db.md 的记载一致 ────────────────────────
if (has('doc/db.md')) {
  const db = read('doc/db.md')
  const { readdirSync } = await import('node:fs')
  const migrations = existsSync(join(ROOT, 'backend/migrations'))
    ? readdirSync(join(ROOT, 'backend/migrations')).filter((f) => f.endsWith('.sql'))
    : []
  const missing = migrations.filter((m) => !db.includes(m))
  if (missing.length) fail(`db.md 未记载迁移：${missing.join(', ')}`)
  else if (migrations.length) ok(`db.md 已记载全部 ${migrations.length} 个迁移`)
}

// ── 输出 ─────────────────────────────────────────────────
const line = '─'.repeat(58)
console.log(line)
for (const p of pass) console.log(`  \u001b[32m✓\u001b[0m ${p}`)
for (const w of warns) console.log(`  \u001b[33m!\u001b[0m ${w}`)
for (const f of fails) console.log(`  \u001b[31m✗\u001b[0m ${f}`)
console.log(line)

if (fails.length) {
  console.log(`\u001b[31m${fails.length} 项不一致\u001b[0m，${pass.length} 项通过`)
  process.exit(1)
}
console.log(`\u001b[32m全部通过\u001b[0m（${pass.length} 项）`)
process.exit(0)