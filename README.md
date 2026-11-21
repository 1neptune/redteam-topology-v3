# RedTeam Atlas

红队攻防资产与攻击路径拓扑平台。把渗透过程画成一张可交付的拓扑图——资产属性、漏洞记录、变更审计与渗透报告都在同一个项目里，不再散落在表格与截图之间。

当前版本 3.4.0  [更新日志](CHANGELOG.md)

---

## 主要功能

**项目与状态**：按项目组织工作，每个项目有进行中 / 已完成两种状态，成员按 owner / editor / viewer 三档授权。

**攻击路径画布**：十六种内置资产组件，拖拽到无限画布；节点间拖连接点建立链路。资产属性覆盖 IP、端口、域名、操作系统、自定义字段与四档控制状态。支持撤销、一键格式化，可导出 PNG（先预览再保存）与 `.atlas.json` 配置。

**漏洞与报告**：一个资产可挂多条漏洞，每条含名称、风险等级、漏洞说明、渗透测试详情（富文本，可插图）与处置建议。报告可预览、导出 Word 与 PDF。

**审计与日志**：每次保存产生字段级审计，记录「谁在什么时候改了哪个对象的哪个字段」；接口访问、运行错误与项目变更各有独立日志文件。

**AI Agent 接入**：内置 MCP 服务端，19 个工具，Agent 可直接读写拓扑。

### 项目列表

按权限过滤的项目清单，含状态、资产数、链路数与最近更新时间。

![项目列表](doc/images/projects.png)

### 画布

左侧组件库、中间无限画布、右侧属性与审计面板。节点卡片显示地址、状态与自定义字段，链路上可直接标注。

![画布](doc/images/canvas.png)

### 渗透测试报告

白底 A4 版面：抬头信息表、漏洞汇总表与逐条详情。预览页里可直接导出 Word 与 PDF。

![渗透测试报告](doc/images/report.png)

---

## 主要技术栈

| 层 | 选型 |
| --- | --- |
| 前端 | Vue 3.5、TypeScript 5.7、Vite 6、Pinia 2、Vue Router 4 |
| 画布 | Vue Flow 1.42 |
| 后端 | Rust 2021、axum 0.8、tokio 1 |
| 数据 | SQLite（sqlx 0.8）、AES-256-GCM 加密拓扑 |
| 鉴权 | JWT、argon2 |
| 导出 | html-to-image、jspdf、docx |

## 前端

单页应用，构建产物是静态文件，由后端统一托管，不需要单独的 Nginx。分层为 `types → api → stores → views → components`，三个页面：登录、项目列表、画布。

## 后端

分层为 `router → handler → service → db → model`，统一响应 `{ code, msg, data }`。除登录与正文图片外，所有接口都要求登录。

## 数据

一个数据目录装下全部状态：SQLite 库、加密主密钥、上传文件与日志。拓扑内容用 AES-256-GCM 加密后入库，密钥来自 `ATLAS_SECRET` 或生成的 `secret.key`。

## 鉴权

JWT 默认 7 天，支持 `Bearer` 与 MCP 用的 HTTP Basic 两种。口令策略为至少 8 位且含大小写。管理员可见全部项目，普通成员只看被授权的项目。

## UI

界面遵循 Hallmark 的现代简约方向：深色单页，三个页面各有明确重心；颜色、字号、间距与控件高度全部走设计令牌，不写死颜色；一个主按钮样式，其余为次级与幽灵按钮；控件高度统一，圆角一层一套。

---

## Windows 开发环境部署运行

**前置**：Node.js 22、Rust stable。未装 MSVC 链接器时切 GNU 工具链：

```powershell
rustup install stable-x86_64-pc-windows-gnu
```

**1. 配置**

复制 `.env.example` 为 `.env`。路径必须用正斜杠（`ATLAS_DATA_DIR=D:/atlas-data`）；`dotenvy` 遇到反斜杠或非 ASCII 注释会整份跳过且不报错。

**2. 启动后端**

```powershell
cd backend
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
$env:ATLAS_DIST_DIR = "$PWD\..\frontend\dist"
cargo run
```

监听 `:8080`，启动日志会打印初始管理员口令。

**3. 启动前端**

```powershell
cd frontend
npm install
npm run dev
```

Vite 带热更新，`/api` 与 `/mcp` 代理到 `8080`。

访问终端里打印的地址。

**4. 文档一致性检查**

改动接口、MCP 工具或版本号之后跑一遍，确认文档没和代码脱节：

```powershell
node scripts/check-docs.mjs
```

检查八项：版本号六处是否一致、CHANGELOG 顶部版本、openapi 与源码路由是否对齐、mcp.md 与源码工具、srs 功能点编号、已移除功能是否还有残留、README 图片引用、迁移文件是否都记进了 db.md。有不一致时退出码为 1，可直接挂到 CI 或提交前钩子。

**5. 一键启动脚本**

```powershell
.\scripts\run.ps1
```

`run.ps1` 必须存成 UTF-8 **带 BOM**，否则 PowerShell 会按本地代码页解析导致语法错误。

---

## 服务器生产环境 Docker 部署

用仓库自带的一键脚本，从零到能访问只跑一条命令。

```bash
git clone https://github.com/1neptune/redteam-topology-v3.git /opt/atlas
cd /opt/atlas
chmod +x scripts/deploy.sh        # 首次需要赋执行权限

sudo ./scripts/deploy.sh
```

交互式问三项（端口、管理员用户名、初始口令），其余全自动。不想回答问题加 `--yes`。

**脚本做的事**

1. 检查 Docker 与 compose 是否可用、Docker 守护进程是否在跑、当前目录是否为项目根
2. 生成 `.env`：随机 64 位十六进制填充 `ATLAS_JWT_SECRET` 与 `ATLAS_SECRET`；口令留空则自动生成一个满足「8 位含大小写」的 16 位口令
3. 创建宿主机数据目录 `/data/atlas`（已存在则跳过，不动里面的数据）
4. 端口占用检测，被占时直接报出占用进程
5. `docker compose build --pull` 构建镜像
6. `docker compose up -d` 启动容器
7. 轮询健康检查 80 秒，不通就打印最近 60 行日志并中止
8. 打印访问地址、管理员账号、初始口令与常用命令

**参数**

| 参数 | 作用 |
| --- | --- |
| `--yes` / `-y` | 全部用默认值，不交互 |
| `--recreate` | 配置已存在也重新生成 `.env`（会换密钥，旧配置自动备份） |
| `--status` | 只看容器状态与健康检查结果 |

**可用环境变量**（不设则交互或取默认）

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `ATLAS_PORT` | `8080` | 对外端口 |
| `ATLAS_ADMIN_USER` | `admin` | 初始管理员用户名 |
| `ATLAS_ADMIN_PASSWORD` | 自动生成 | 初始管理员口令 |
| `ATLAS_DATA_DIR_HOST` | `/data/atlas` | 宿主机数据目录 |
| `ATLAS_JWT_SECRET` | 自动生成 | JWT 签名密钥 |
| `ATLAS_SECRET` | 自动生成 | 拓扑加密主密钥 |

非交互部署：

```bash
sudo ATLAS_PORT=8080 ATLAS_ADMIN_PASSWORD='Your#Pass2026' ./scripts/deploy.sh --yes
```

**构建慢怎么办**

首次构建要下载并编译全部 Rust 依赖，3–6 分钟属正常。如果每次都卡在 `apt-get install` 上百秒，是包源线路问题，在 `.env` 里指到国内镜像：

```bash
APT_MIRROR=mirrors.aliyun.com
NPM_REGISTRY=https://registry.npmmirror.com
CARGO_MIRROR=sparse+https://mirrors.ustc.edu.cn/crates.io-index/
```

加完重新构建，apt 阶段通常从 130 秒降到十几秒：

```bash
docker compose build --no-cache atlas
```

只有第一次需要 `--no-cache`，之后依赖没变会命中缓存直接跳过。

镜像用了 BuildKit 的缓存挂载（`--mount=type=cache`），需要 Docker 20.10 以上并启用 BuildKit（Docker Desktop 与新版 Docker Engine 默认开启）。

**幂等**

重复执行不会破坏已有数据：`.env` 已存在就原样保留（密钥不变，历史拓扑仍可解密），数据目录已存在就跳过，容器用 `up -d` 覆盖而非重建。要换密钥得显式加 `--recreate`。

**手动做法**（脚本不可用时）

```bash
cat > .env <<'EOF'
ATLAS_PORT=8080
ATLAS_JWT_SECRET=换成32位以上随机串
ATLAS_SECRET=换成主密钥口令
ATLAS_ADMIN_USER=admin
ATLAS_ADMIN_PASSWORD=换成满足规则的初始口令
EOF

docker compose build
docker compose up -d
docker compose logs -f atlas     # 启动日志里有初始管理员口令
```

`ATLAS_JWT_SECRET` 为空时每次启动随机生成，重启后所有人被登出；`ATLAS_SECRET` 决定拓扑能否解密，换掉等于让历史拓扑不可读。

数据落在宿主机 `/data/atlas`，由容器入口脚本修正属主后降权运行，目录无需预先创建。

---

## 服务器生产环境 Docker 升级

用仓库自带的脚本，一条命令做完备份、构建、重启、健康检查，失败自动回滚。

```bash
chmod +x scripts/upgrade.sh     # 首次需要赋执行权限

./scripts/upgrade.sh --check    # 只看当前版本与远端最新版本
./scripts/upgrade.sh            # 升级到远端最新
./scripts/upgrade.sh v4.0.0     # 钉到指定 tag
./scripts/upgrade.sh --rollback # 回滚到上一次升级前
```

**脚本做的事**

1. 把 `/data/atlas` 打成 `backups/<时间戳>/atlas-data.tar.gz`，同时备份 `.env` 与 `docker-compose.yml`，保留最近 5 份
2. 记下当前镜像 id 与 git 提交，写进 `.upgrade-state`，作为回滚依据
3. `git fetch --tags` 后 `git pull --ff-only`（或切到指定 tag）
4. `docker compose build --pull` 重建镜像，`up -d` 重启
5. 轮询 `http://127.0.0.1:<ATLAS_PORT>/` 最多 60 秒等就绪
6. 健康检查不过就自动 `--rollback`，把升级前的镜像重新拉起

**可用环境变量**

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `BACKUP_DIR` | `./backups` | 备份存放目录 |
| `KEEP_BACKUPS` | `5` | 保留几份备份 |
| `HEALTH_TRIES` | `30` | 健康检查次数，每次间隔 2 秒 |
| `HEALTH_URL` | `http://127.0.0.1:<port>/` | 自定义探测地址 |

**手动做法**（脚本不可用时）

```bash
docker compose exec atlas tar czf - /data > atlas-backup-$(date +%F).tar.gz
git pull
docker compose build
docker compose up -d
docker compose logs -f atlas
```

数据库迁移在启动时自动执行，无需手工跑 SQL。已应用的迁移文件不能再改，要调整就新增一个迁移。

**升级只有一种方式：命令行**

界面不提供「检查更新 / 一键更新」。容器无法重建自身，界面上点一下也只能提示去宿主机执行；二进制部署的替换重启又依赖进程退出，容易做成半截状态。统一用脚本，行为可预期、可回滚、有日志。

`GET /api/system/version` 只返回当前版本与仓库地址，不联网、不判断有没有新版本。

---

## 版本发布

版本号取自 `CHANGELOG.md` 与 GitHub Releases，两处必须一致。每次功能改动都走一遍：

**1. 更新 CHANGELOG.md**

在顶部加一节，按「新增 / 变更 / 修复」分类：

```markdown
## v4.1.0

### 新增
- ...
```

**2. 同步版本号**

六处一起改，漏一处就会出现「界面显示一个版本、镜像标签是另一个」：

```
backend/Cargo.toml          version = "4.1.0"
frontend/package.json       "version": "4.1.0"
docker-compose.yml          image: redteam-atlas:4.1.0
Dockerfile                  org.opencontainers.image.version="4.1.0"
doc/openapi.md              version: 4.1.0
doc/mcp.md                  serverInfo.version: 4.1.0
```

**3. 提交并打 tag**

```bash
git add -A
git commit -m "feat: 三句话说明改了什么"
git tag -a v4.1.0 -m "v4.1.0"
git push origin master --tags
```

**4. 建 Release**

到 <https://github.com/1neptune/redteam-topology-v3/releases> 新建 Release，选刚推的 tag，正文直接粘 CHANGELOG 里那一节。

必须带 `v` 前缀的 tag——`upgrade.sh --check` 靠 `git ls-remote --tags` 里 `v*` 的标签排序判断远端版本，不带前缀会漏掉。

**不要勾 Pre-release**：`upgrade.sh --check` 用 `git ls-remote --tags` 的最新高版本号判断远端，不带 `v` 前缀或标成预发布都会被漏掉。

不想点网页的话用 API 建，需要一次 `repo` 权限的 token：

```bash
TAG=v4.1.0
BODY=$(awk "/^## $TAG\$/,/^## v[0-9]/" CHANGELOG.md | sed '$d' | tail -n +2)

curl -sS -X POST \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  https://api.github.com/repos/1neptune/redteam-topology-v3/releases \
  -d "{\"tag_name\":\"$TAG\",\"name\":\"$TAG\",\"body\":$(printf '%s' "$BODY" | jq -Rs .),\"draft\":false,\"prerelease\":false}"
```

建完验一下，返回的 `tag_name` 应该就是你刚发布的版本：

```bash
curl -sS https://api.github.com/repos/1neptune/redteam-topology-v3/releases/latest \
  | grep -o '"tag_name": *"[^"]*"'
```

**版本号怎么定**

| 改动 | 版本 |
| --- | --- |
| 加功能、加接口、加工具 | 次版本 +1（4.0.0 → 4.1.0） |
| 只修 bug、改文案、调样式 | 修订号 +1（4.0.0 → 4.0.1） |
| 破坏兼容：改接口字段、改数据表、换加密方式 | 主版本 +1（4.0.0 → 5.0.0） |

数据库迁移算破坏兼容——旧数据不迁移就跑不起来，主版本 +1，并在 CHANGELOG 里写清楚迁移步骤。

---

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [doc/srs.md](doc/srs.md) | 需求规格：业务模块、70 条功能点、非功能要求、目录结构 |
| [doc/openapi.md](doc/openapi.md) | 接口文档：36 个 REST 接口的参数、返回与错误 |
| [doc/mcp.md](doc/mcp.md) | MCP 接入：握手、鉴权、19 个工具、字段取值与典型工作流 |
| [doc/db.md](doc/db.md) | 数据库：表结构、索引、迁移说明 |
| [CHANGELOG.md](CHANGELOG.md) | 版本更新日志 |