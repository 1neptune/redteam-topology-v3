/** 版本与更新检查 */

export interface VersionInfo {
  /** 当前运行版本 */
  current: string
  /** 可用版本（本地清单或 GitHub Releases） */
  latest: string
  /** 是否存在更新 */
  updateAvailable: boolean
  /** 更新说明 */
  notes: string
  /** 清单发布时间 */
  publishedAt: number | null
  /** 来源：manifest（本地 update.json）/ none */
  source: string
  /** 官网仓库 owner/name */
  repo: string
  /** 官网地址 */
  homepage: string
  /** 发布页（更新日志） */
  releasesUrl: string
  /** 查询最新版本的 GitHub API 地址 */
  checkUrl: string
  /** 本次检查时间 */
  checkedAt: number
  /** 升级动作提示 */
  upgradeHint: string[]
}

/** GitHub Releases 的最新一条 */
export interface GithubRelease {
  /** 形如 v3.3.0 */
  tagName: string
  /** 发布标题 */
  name: string
  /** 发布说明（Markdown） */
  body: string
  /** 发布页地址 */
  htmlUrl: string
  /** 发布时间（ISO 字符串） */
  publishedAt: string
}
/** 一键更新的结果 */
export interface UpgradeResult {
  from: string
  to: string
  /** 下载到的资产文件名 */
  asset: string
  /** 落盘路径 */
  staged: string
  /** 字节数 */
  bytes: number
  /** 替换脚本路径 */
  script: string
  /** 是否已拉起重启脚本 */
  restarting: boolean
}