/** 版本信息：只读，不含任何「有没有新版本」的判断 */
export interface VersionInfo {
  /** 当前运行版本 */
  current: string
  /** 仓库 owner/name */
  repo: string
  /** 仓库主页 */
  homepage: string
  /** 发布页 */
  releasesUrl: string
  /** 命令行升级方式 */
  upgradeHint: string[]
  /** 本次查询时间 */
  checkedAt: number
}