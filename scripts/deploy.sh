#!/usr/bin/env bash
#
# RedTeam Atlas 服务器生产环境一键部署脚本（Docker）
#
# 做的事：检查环境 → 生成配置 → 准备数据目录 → 构建镜像 → 启动 → 健康检查 → 打印入口。
#
# 用法：
#   ./scripts/deploy.sh              交互式部署（缺配置会问）
#   ./scripts/deploy.sh --yes        全部用默认值，不提问
#   ./scripts/deploy.sh --recreate   配置已存在也重新生成 .env（会换掉密钥，历史拓扑将不可读）
#   ./scripts/deploy.sh --status     只看运行状态
#
# 环境变量（不设则用默认值或交互输入）：
#   ATLAS_PORT            对外端口，默认 8080
#   ATLAS_ADMIN_USER      初始管理员用户名，默认 admin
#   ATLAS_ADMIN_PASSWORD  初始管理员口令，留空则自动生成
#   ATLAS_DATA_DIR_HOST   宿主机数据目录，默认 /data/atlas
#   ATLAS_JWT_SECRET      JWT 签名密钥，留空则自动生成
#   ATLAS_SECRET          拓扑加密主密钥，留空则自动生成

set -Eeuo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

ENV_FILE="$REPO_ROOT/.env"
ASSUME_YES=0
FORCE_CONFIG=0
HEALTH_TRIES="${HEALTH_TRIES:-40}"

c_reset=$'\033[0m'
c_info=$'\033[36m'
c_ok=$'\033[32m'
c_warn=$'\033[33m'
c_err=$'\033[31m'

log()  { printf '%s[%s]%s %s\n' "$c_info" "$(date +%H:%M:%S)" "$c_reset" "$*"; }
ok()   { printf '%s[%s]%s %s\n' "$c_ok"   "$(date +%H:%M:%S)" "$c_reset" "$*"; }
warn() { printf '%s[%s]%s %s\n' "$c_warn" "$(date +%H:%M:%S)" "$c_reset" "$*" >&2; }
die()  { printf '%s[%s]%s %s\n' "$c_err"  "$(date +%H:%M:%S)" "$c_reset" "$*" >&2; exit 1; }

trap 'die "第 $LINENO 行失败，已中止"' ERR

# ── 前置检查 ───────────────────────────────────────────────
need() { command -v "$1" >/dev/null 2>&1 || die "缺少命令：$1"; }

check_env() {
  need docker
  docker compose version >/dev/null 2>&1 \
    || command -v docker-compose >/dev/null 2>&1 \
    || die "需要 docker compose 或 docker-compose"

  docker info >/dev/null 2>&1 \
    || die "Docker 守护进程没在跑，或当前用户没权限（试 sudo 或把用户加进 docker 组）"

  [ -f Dockerfile ] || die "当前目录不是项目根：找不到 Dockerfile"
  [ -f docker-compose.yml ] || die "当前目录不是项目根：找不到 docker-compose.yml"

  if [ "$(id -u)" -ne 0 ] && [ ! -w "$REPO_ROOT" ]; then
    die "对 $REPO_ROOT 没有写权限，请用能写的用户或 sudo 运行"
  fi
  ok "环境检查通过"
}

compose() {
  if docker compose version >/dev/null 2>&1; then
    docker compose "$@"
  else
    docker-compose "$@"
  fi
}

# ── 随机值与口令 ───────────────────────────────────────────
rand_hex() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex "$1"
  else
    head -c "$1" /dev/urandom | od -An -tx1 | tr -d ' \n'
  fi
}

# 至少 8 位且同时含大写、小写；这里给 20 位，含数字与符号
gen_password() {
  local upper lower digits symbol
  upper="$(tr -dc 'A-Z' </dev/urandom | head -c 4)"
  lower="$(tr -dc 'a-z' </dev/urandom | head -c 6)"
  digits="$(tr -dc '0-9' </dev/urandom | head -c 4)"
  symbol="$(tr -dc '@#%_+=' </dev/urandom | head -c 2)"
  printf '%s%s%s%s' "$upper" "$lower" "$digits" "$symbol"
}

ask() {
  # ask <变量名> <提示> <默认值>
  local __var="$1" __prompt="$2" __default="$3" __reply
  if [ "$ASSUME_YES" -eq 1 ] || [ ! -t 0 ]; then
    printf -v "$__var" '%s' "$__default"
    return 0
  fi
  read -r -p "$__prompt [$__default]: " __reply || true
  printf -v "$__var" '%s' "${__reply:-$__default}"
}

# ── 配置生成 ───────────────────────────────────────────────
write_env() {
  local port admin_user admin_pass jwt secret data_dir

  port="${ATLAS_PORT:-8080}"
  admin_user="${ATLAS_ADMIN_USER:-admin}"
  data_dir="${ATLAS_DATA_DIR_HOST:-/data/atlas}"

  if [ -f "$ENV_FILE" ] && [ "$FORCE_CONFIG" -eq 0 ]; then
    ok ".env 已存在，保留原有配置（密钥不变，历史拓扑仍可解密）"
    # shellcheck disable=SC1090
    port="$(grep -E '^ATLAS_PORT=' "$ENV_FILE" | tail -n1 | cut -d= -f2 | tr -d '"'"'"' ' || true)"
    port="${port:-8080}"
    return 0
  fi

  if [ -f "$ENV_FILE" ] && [ "$FORCE_CONFIG" -eq 1 ]; then
    local backup
    backup="$ENV_FILE.bak.$(date +%Y%m%d-%H%M%S)"
    cp "$ENV_FILE" "$backup"
    warn "已重新生成配置，旧配置备份在 $backup"
    warn "换了 ATLAS_SECRET 后，此前的拓扑将无法解密"
  fi

  ask port "对外端口" "$port"
  ask admin_user "初始管理员用户名" "$admin_user"
  admin_pass="${ATLAS_ADMIN_PASSWORD:-}"
  if [ -z "$admin_pass" ]; then
    if [ "$ASSUME_YES" -eq 0 ] && [ -t 0 ]; then
      read -r -p "初始管理员口令（留空自动生成，至少 8 位含大小写）: " admin_pass || true
    fi
    [ -n "$admin_pass" ] || admin_pass="$(gen_password)"
  fi
  jwt="${ATLAS_JWT_SECRET:-$(rand_hex 32)}"
  secret="${ATLAS_SECRET:-$(rand_hex 32)}"

  umask 077
  cat >"$ENV_FILE" <<EOF
# RedTeam Atlas 生产配置，由 scripts/deploy.sh 生成于 $(date -Iseconds)
# 这个文件含密钥，不要进版本库（.gitignore 已排除）

# 对外端口
ATLAS_PORT=$port

# JWT 签名密钥：换掉等于让所有已登录会话失效
ATLAS_JWT_SECRET=$jwt

# 拓扑加密主密钥：换掉等于让所有历史拓扑不可读，务必连同数据一起备份
ATLAS_SECRET=$secret

# 初始管理员：仅在用户表为空时生效
ATLAS_ADMIN_USER=$admin_user
ATLAS_ADMIN_PASSWORD=$admin_pass
EOF
  umask 022

  ok "已生成 .env（端口 $port，管理员 $admin_user）"
  ADMIN_PASSWORD_GENERATED="$admin_pass"
}

prepare_data_dir() {
  local data_dir="${ATLAS_DATA_DIR_HOST:-/data/atlas}"
  if [ ! -d "$data_dir" ]; then
    mkdir -p "$data_dir" 2>/dev/null || die "无法创建 $data_dir，请用 sudo 运行或改 ATLAS_DATA_DIR_HOST"
    ok "已创建数据目录 $data_dir"
  else
    local size
    size="$(du -sh "$data_dir" 2>/dev/null | cut -f1 || echo '?')"
    ok "数据目录已存在：$data_dir（$size）"
  fi
}

check_port() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    if ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]$port\$"; then
      local holder
      holder="$(ss -ltnp 2>/dev/null | grep -E "[:.]$port\$" | head -n1 || true)"
      die "端口 $port 已被占用：$holder
换个端口：ATLAS_PORT=8081 ./scripts/deploy.sh"
    fi
  fi
}

# ── 版本清单 ───────────────────────────────────────────────
# 容器内往往连不上 GitHub，「检查更新」读这份清单来判断有没有新版本。
write_manifest() {
  local version="$1"
  local dir="${ATLAS_DATA_DIR_HOST:-/data/atlas}"
  [ -d "$dir" ] || return 0
  cat > "$dir/update.json" <<EOF
{
  "version": "$version",
  "notes": "",
  "publishedAt": $(date +%s)000
}
EOF
  chmod 644 "$dir/update.json" 2>/dev/null || true
  ok "已写入版本清单 $dir/update.json（版本 $version）"
}
# ── 部署 ──────────────────────────────────────────────────
wait_healthy() {
  local url="http://127.0.0.1:$1/"
  log "等待服务就绪：$url"
  local i code
  for ((i = 1; i <= HEALTH_TRIES; i++)); do
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "$url" 2>/dev/null || true)"
    if [ "$code" = "200" ]; then
      ok "服务已就绪（第 $i 次探测）"
      return 0
    fi
    sleep 2
  done
  return 1
}

do_deploy() {
  check_env
  write_env
  prepare_data_dir

  local port
  port="$(grep -E '^ATLAS_PORT=' "$ENV_FILE" | tail -n1 | cut -d= -f2 | tr -d '"'"'"' ')"
  port="${port:-8080}"
  check_port "$port"

  log "构建镜像（首次约 5–10 分钟，之后走缓存通常在 1 分钟内）"
  compose build --pull atlas

  log "启动容器"
  compose up -d --remove-orphans

  if ! wait_healthy "$port"; then
    warn "健康检查未通过，容器状态："
    compose ps >&2 || true
    warn "最近日志："
    compose logs --tail=80 --no-color atlas >&2 2>&1 || true
    warn "容器还没起来时 compose logs 可能是空的，用下面这条直接取："
    warn "  docker logs --tail 80 redteam-atlas"
    die "部署失败。修复后重跑 ./scripts/deploy.sh，或 ./scripts/upgrade.sh --rollback"
  fi

  local ip
  ip="$(hostname -I 2>/dev/null | awk '{print $1}' || echo '<服务器IP>')"

  printf '\n'
  ok "部署完成"
  printf '  访问地址   http://%s:%s\n' "${ip:-<服务器IP>}" "$port"
  printf '  管理员     %s\n' "$(grep -E '^ATLAS_ADMIN_USER=' "$ENV_FILE" | cut -d= -f2)"
  if [ -n "${ADMIN_PASSWORD_GENERATED:-}" ]; then
    printf '  初始口令   %s\n' "$ADMIN_PASSWORD_GENERATED"
    warn "这个口令只显示一次，请立即登录并改掉"
  else
    printf '  初始口令   见 .env 的 ATLAS_ADMIN_PASSWORD（已有账号时不生效）\n'
  fi
  # 首次部署就把当前版本写进清单，后续「检查更新」才有对比基准
  local cur
  cur="$(grep -m1 '^version' backend/Cargo.toml 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo '')"
  [ -n "$cur" ] && write_manifest "$cur"
  printf '  版本       %s\n' "${cur:-未知}"
  printf '  数据目录   %s\n' "${ATLAS_DATA_DIR_HOST:-/data/atlas}"
  printf '\n'
  printf '常用命令：\n'
  printf '  docker compose logs -f atlas     # 跟日志\n'
  printf '  docker compose ps                # 状态\n'
  printf '  ./scripts/upgrade.sh             # 升级\n'
}

do_status() {
  check_env
  compose ps
  printf '\n'
  if [ -f "$ENV_FILE" ]; then
    local port
    port="$(grep -E '^ATLAS_PORT=' "$ENV_FILE" | tail -n1 | cut -d= -f2 | tr -d '"'"'"' ')"
    local code
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "http://127.0.0.1:${port:-8080}/" 2>/dev/null || true)"
    printf '健康检查   HTTP %s\n' "${code:-无响应}"
  else
    warn "还没有 .env，尚未部署"
  fi
}

# ── 入口 ─────────────────────────────────────────────────
while [ $# -gt 0 ]; do
  case "$1" in
    --yes|-y)  ASSUME_YES=1 ;;
    --recreate) FORCE_CONFIG=1 ;;
    --status)  do_status; exit 0 ;;
    -h|--help) sed -n '3,23p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)         die "未知参数：$1（用 --help 看用法）" ;;
  esac
  shift
done

do_deploy