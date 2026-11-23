#!/usr/bin/env bash
#
# RedTeam Atlas 服务器生产环境升级脚本（Docker）
#
# 做的事：备份数据 → 记录当前状态 → 取新代码 → 重建镜像 → 重启 →
#         等健康检查 → 失败自动回滚到升级前的镜像。
#
# 用法：
#   ./scripts/upgrade.sh                 升级到远端最新代码
#   ./scripts/upgrade.sh v3.5.0          升级到指定 tag
#   ./scripts/upgrade.sh --rollback      回滚到上一次升级前的状态
#   ./scripts/upgrade.sh --check         只看有没有新版本，不做改动
#
# 环境变量：
#   BACKUP_DIR   备份落在哪，默认 ./backups
#   KEEP_BACKUPS 保留几份备份，默认 5
#   HEALTH_URL   健康检查地址，默认 http://127.0.0.1:${ATLAS_PORT:-8080}/
#   HEALTH_TRIES 健康检查最多试几次，默认 30（每次间隔 2 秒）

set -Eeuo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BACKUP_DIR="${BACKUP_DIR:-$REPO_ROOT/backups}"
KEEP_BACKUPS="${KEEP_BACKUPS:-5}"
HEALTH_TRIES="${HEALTH_TRIES:-30}"
STATE_FILE="$REPO_ROOT/.upgrade-state"

c_reset=$'\033[0m'
c_info=$'\033[36m'
c_ok=$'\033[32m'
c_warn=$'\033[33m'
c_err=$'\033[31m'

log()  { printf '%s[%s]%s %s\n' "$c_info" "$(date +%H:%M:%S)" "$c_reset" "$*"; }
ok()   { printf '%s[%s]%s %s\n' "$c_ok"   "$(date +%H:%M:%S)" "$c_reset" "$*"; }
warn() { printf '%s[%s]%s %s\n' "$c_warn" "$(date +%H:%M:%S)" "$c_reset" "$*" >&2; }
die()  { printf '%s[%s]%s %s\n' "$c_err"  "$(date +%H:%M:%S)" "$c_reset" "$*" >&2; exit 1; }

trap 'die "第 $LINENO 行失败，已中止（数据未被修改前的备份见 $BACKUP_DIR）"' ERR

# ── 前置检查 ───────────────────────────────────────────────
need() { command -v "$1" >/dev/null 2>&1 || die "缺少命令：$1"; }
need docker
need git

compose() {
  if docker compose version >/dev/null 2>&1; then
    docker compose "$@"
  elif command -v docker-compose >/dev/null 2>&1; then
    docker-compose "$@"
  else
    die "找不到 docker compose，也没找到 docker-compose"
  fi
}

port="${ATLAS_PORT:-8080}"
if [ -f .env ]; then
  # 只读一行，不 source 整个文件，避免执行任意内容
  env_port="$(grep -E '^ATLAS_PORT=' .env | tail -n1 | cut -d= -f2 | tr -d '"'"'"' ' || true)"
  [ -n "${env_port:-}" ] && port="$env_port"
fi
HEALTH_URL="${HEALTH_URL:-http://127.0.0.1:$port/}"

md5_of() { md5sum "$1" 2>/dev/null | cut -d' ' -f1 || sha1sum "$1" | cut -d' ' -f1; }

# ── 状态记录（回滚依据）────────────────────────────────────
save_state() {
  # 故意写成全局：升级流程紧接着要用 IMAGE_ID 打日志，回滚再从文件读回来
  IMAGE_ID="$(compose images -q atlas 2>/dev/null | head -n1 || true)"
  GIT_REF="$(git rev-parse --short HEAD 2>/dev/null || echo none)"
  cat >"$STATE_FILE" <<EOF
IMAGE_ID=$IMAGE_ID
GIT_REF=$GIT_REF
SAVED_AT=$(date -Iseconds)
EOF
}

load_state() {
  [ -f "$STATE_FILE" ] || die "没有升级记录，无法回滚"
  # shellcheck disable=SC1090
  . "$STATE_FILE"
}

# ── 备份 ──────────────────────────────────────────────────
backup() {
  mkdir -p "$BACKUP_DIR"
  local stamp dir
  stamp="$(date +%Y%m%d-%H%M%S)"
  dir="$BACKUP_DIR/$stamp"
  mkdir -p "$dir"

  log "备份数据目录 /data/atlas"
  # 数据在宿主机目录，直接打包最省事；容器里没有 tar 也不影响
  if [ -d /data/atlas ]; then
    tar czf "$dir/atlas-data.tar.gz" -C /data atlas 2>/dev/null \
      || die "备份 /data/atlas 失败，检查读权限"
  else
    # 数据目录不在默认位置时退回容器内打包
    compose exec -T atlas tar czf - /data > "$dir/atlas-data.tar.gz" \
      || die "备份 /data 失败"
  fi

  for f in .env docker-compose.yml; do
    [ -f "$f" ] && cp "$f" "$dir/" || true
  done
  git rev-parse HEAD > "$dir/git-ref.txt" 2>/dev/null || true

  local size
  size="$(du -h "$dir/atlas-data.tar.gz" | cut -f1)"
  ok "备份完成：$dir（$size）"

  # 只保留最近若干份
  ls -1dt "$BACKUP_DIR"/*/ 2>/dev/null | tail -n +$((KEEP_BACKUPS + 1)) | xargs -r rm -rf
  echo "$dir" > "$BACKUP_DIR/.latest"
}

# ── 健康检查 ───────────────────────────────────────────────
wait_healthy() {
  log "等待服务就绪：$HEALTH_URL"
  local i code
  for ((i = 1; i <= HEALTH_TRIES; i++)); do
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "$HEALTH_URL" || true)"
    if [ "$code" = "200" ]; then
      ok "服务已就绪（第 $i 次探测）"
      return 0
    fi
    sleep 2
  done
  return 1
}

# ── 版本信息 ───────────────────────────────────────────────
current_version() {
  local v
  # 容器在跑就问它自己；--version 会立刻返回，不会启动服务
  v="$(compose exec -T atlas /usr/local/bin/redteam-atlas --version 2>/dev/null \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n1 || true)"
  if [ -z "$v" ]; then
    # 容器没起来时读源码里的版本，这是即将构建的版本
    v="$(grep -m1 '^version' backend/Cargo.toml 2>/dev/null \
      | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true)"
  fi
  echo "${v:-unknown}"
}

remote_version() {
  local tag
  tag="$(git ls-remote --tags --refs origin 'v*' 2>/dev/null \
    | sed 's#.*refs/tags/##' | sort -V | tail -n1)"
  echo "${tag:-unknown}"
}

# ── 命令实现 ───────────────────────────────────────────────
do_check() {
  local cur rem
  cur="$(current_version)"
  rem="$(remote_version)"
  log "当前版本：$cur"
  log "远端最新：$rem"
  if [ "$cur" = "$rem" ]; then
    ok "已是最新"
  else
    warn "有新版本可用：$cur → $rem"
  fi
}

do_rollback() {
  load_state
  [ -n "${IMAGE_ID:-}" ] || die "升级记录里没有镜像 id，无法回滚"
  warn "回滚到升级前的镜像 ${IMAGE_ID:0:12}（git ${GIT_REF:-unknown}）"
  docker tag "$IMAGE_ID" redteam-atlas:rollback
  compose down
  compose up -d --no-build || die "回滚启动失败"
  wait_healthy || die "回滚后健康检查仍未通过，请查看 docker compose logs"
  ok "已回滚"
}

do_upgrade() {
  local target="${1:-}"
  local cur rem
  cur="$(current_version)"
  log "当前版本：$cur"

  backup
  save_state
  local shown_image
  if [ -n "${IMAGE_ID:-}" ]; then
    shown_image="${IMAGE_ID:0:12}"
  else
    shown_image="（无，尚未构建过镜像）"
  fi
  log "已记录升级前状态：镜像 $shown_image，git ${GIT_REF:-unknown}"

  log "取新代码"
  git fetch --tags --prune origin
  if [ -n "$target" ]; then
    git checkout "$target" 2>/dev/null || die "找不到 tag：$target"
    ok "已切到 $target"
  else
    local branch
    branch="$(git rev-parse --abbrev-ref HEAD)"
    git pull --ff-only origin "$branch" || die "拉取失败，本地有未提交改动？"
    ok "已更新到 $(git rev-parse --short HEAD)"
  fi

  log "重建镜像并重启"
  compose build --pull atlas || die "镜像构建失败"
  compose up -d --remove-orphans || die "启动失败"

  if wait_healthy; then
    ok "升级完成：$cur → $(current_version)"
    log "备份保留在：$(cat "$BACKUP_DIR/.latest" 2>/dev/null || echo "$BACKUP_DIR")"
    return 0
  fi

  warn "健康检查未通过，开始回滚"
  do_rollback
  die "升级失败，已回滚到 $cur。排查请执行：docker compose logs --tail=120 atlas"
}

# ── 入口 ─────────────────────────────────────────────────
case "${1:-}" in
  --check)    do_check ;;
  --rollback) do_rollback ;;
  -h|--help)  sed -n '3,26p' "$0" | sed 's/^# \{0,1\}//' ;;
  "")         do_upgrade "" ;;
  -*)         die "未知参数：$1（用 --help 看用法）" ;;
  *)          do_upgrade "$1" ;;
esac