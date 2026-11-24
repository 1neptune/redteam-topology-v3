# RedTeam Atlas · 需求规格说明书（SRS）

> 版本 v3.1.0 · 技术栈：Vue3 + Vite + TypeScript / Rust Axum / SQLite + sqlx / JWT
> 本文档为唯一需求基线，每次对话先读本文再改动代码。

## 1. 业务定位

面向红队与攻防演练团队的攻击路径拓扑平台。以「项目」为单位保存资产与攻击链路，
多人协作、按项目授权、每次改动留痕（谁、何时、改了哪个字段）。

## 2. 业务模块

| 模块 | 职责 |
| --- | --- |
| 认证 Auth | 登录、登出、当前用户、修改口令；系统唯一公开接口是登录 |
| 用户 User | 管理员建号、改角色、启停用、重置口令（不开放自助注册） |
| 项目 Project | 项目 CRUD、搜索、复制、导入导出 JSON、版本号乐观锁 |
| 拓扑 Graph | 节点与连线的加密存储、字段级 diff 计算 |
| 权限 Grant | 项目级 owner/editor/viewer 三级授权，管理员可授权任意项目 |
| 组件 AssetType | 内置 + 自定义组件类型，图标上传与鉴权读取 |
| 审计 Audit | 记录节点/连线/项目/授权变更，按项目与全局查询 |
| MCP | 面向 AI Agent 的 JSON-RPC 工具集，支持 Bearer 与 Basic 两种鉴权 |
| 系统 System | 版本查询（只读当前版本与仓库地址）与运行日志 |

## 3. 功能清单

### 3.1 账户与鉴权
- F-01 系统不开放注册；账号只能由管理员在「用户管理」中创建。
- F-02 口令使用 Argon2id 哈希；登录签发 JWT，有效期默认 7 天。
- F-03 **除 `POST /api/auth/login` 外，所有 `/api` 接口均要求 `Authorization: Bearer <token>`**，缺失或过期返回 401。
- F-04 修改口令后旧 JWT 立即失效（通过 `users.token_version` 自增实现）。
- F-05 管理员可建号、改角色、停用账号、重置口令；不能停用或降级自己。

### 3.2 项目与拓扑
- F-06 项目列表仅返回当前用户可见项目，支持名称/描述搜索（`?q=`）。
- F-07 拓扑以 JSON 序列化后 AES-256-GCM 加密存储，密钥来自环境变量。
- F-08 保存拓扑使用版本号乐观锁，冲突返回 409。
- F-09 导出 `.atlas.json`；导入生成新项目。
- F-10 复制项目（需读权限，副本归操作者）。

### 3.3 画布交互（前端）
- F-11 左侧组件库拖拽到画布创建节点，支持组件搜索。
- F-11b 节点控制状态共四档：侦察中、攻击中、已控制、受阻中；状态按钮按等分网格排列，宽度不随文字长短变化。
- F-12b 从连接点拖出后松手在空白处，会把其余资产组件列成菜单，点一个即建立链路；落在组件上则直接连到该组件。
- F-12 节点左右两侧各有一个连接点，从任一侧按住拖到目标节点的连接点即建立链路（`ConnectionMode.Loose`，方向由拖出的一侧决定）；不设「从这里连线」按钮。
- F-13 点选节点/连线，右侧面板编辑属性与任意自定义字段。
- F-14 导出 PNG（2x，隐藏交互控件并附项目水印）。
- F-15 快捷键：F 适应视图、Delete 删除选中、Esc 取消、Ctrl/⌘+S 保存、Ctrl/⌘+Z 撤销上一步图操作。

### 3.4 团队协作
- F-16 项目内实时在线成员（轮询快照）。
- F-17 他人保存后提示版本变化，可载入最新或用本地覆盖。

### 3.5 授权与审计
- F-18 管理员或项目 owner 可授权成员（editor/viewer/owner）。
- F-19 被授权 editor 可编辑拓扑；仅 owner 可改项目资料与再授权。
- F-20 每次保存产生字段级审计（节点/连线的新增、修改、删除）。
- F-21 节点卡片显示最后编辑者；审计面板显示完整轨迹。

### 3.6 组件与上传
- F-22 自定义组件：名称、标识色、图标。内置 5 个图标（靶标/漏洞/服务器/云资源/凭证，新建时随机预选一个），
  也可上传 PNG/JPEG/WebP/GIF/SVG 替换（≤2MB）。内置图标以 `builtin:<key>` 存储，不占磁盘。
- F-23 上传文件随机命名、按 MIME 白名单校验、独立目录存放、禁止执行。
- F-24 图标读取接口同样要求登录；前端带令牌取流后转对象 URL 显示。
- F-25 组件修改记录操作者；仅创建者或管理员可改删。

### 3.7 MCP / AI Agent
- F-26 `POST /mcp` 支持 initialize / tools/list / tools/call。
- F-27 鉴权支持两种：`Bearer <token>` 与 `Basic base64(用户名:口令)`。
- F-28 工具共 19 个：`list_projects`、`get_project`、`create_project`、`update_project`、`delete_project`、
  `ingest_attack_path`、`add_nodes`、`add_edges`、`update_node`、`delete_node`、`delete_edge`、
  `set_vulnerability`、`remove_vulnerability`、`search_graph`、`list_asset_types`、`create_asset_type`、
  `add_member`、`list_users`、`create_user`。
- F-2a 启动摘要只打印版本、可访问地址（监听 0.0.0.0 时取本机内网 IP）与初始管理员。
- F-2b 口令策略：至少 8 位，且同时包含大写与小写字母；建号、改口令、管理员重置都适用。
- F-2c 建号时可一并选择项目与权限，创建后立即成为该项目成员（需创建人是项目 owner）。
- F-28b `set_vulnerability` 追加或改写一条漏洞（`index` 省略则追加），复用保存流程并产生审计；
  `remove_vulnerability` 按序号删除一条。
- F-29 `ingest_attack_path` 自动分层布局、按 label/外部 id 解析引用、生成审计。

### 3.8 漏洞详情与报告
- F-34 画布节点**双击**打开「渗透测试漏洞详情」抽屉；**单击**只做选中与右侧属性编辑，两者严格分开。
- F-35 一个资产可挂**多条**漏洞；「添加漏洞」追加，列表点选切换，右侧 × 删除，每条是一个完整单元
  （漏洞名称、风险等级、漏洞说明、渗透测试详情、处置建议）。
- F-35b 正文里插图走独立资源：上传后得到带签名的短地址，正文只存该地址，报告预览、Word 与 PDF 都会带上。
- F-36 整组漏洞保存写回节点 `data.vulns`，产生字段级审计（新增/删除整条、逐字段改动，长文本记摘要）。
- F-37 报告支持**预览**：白底 A4 版面，含抬头、汇总表（序号/名称/等级）与逐条详情；预览页内可直接导出。
- F-38 导出 Word（.docx）与 PDF；PDF 由预览 DOM 渲染，中文依赖系统字体，无需嵌入字体文件。
- F-39 节点卡片显示一组漏洞中的最高风险等级徽章，颜色与面板、报告一致。
- F-40 内置组件新增「攻击者」（`attacker`），与攻击入口同属「入口 / 暴露面」分组。

### 3.9 系统与升级
- F-5a 资产属性为 IP、端口、域名、操作系统、备注与自定义字段；状态固定四档「侦察中 / 攻击中 / 已控制 / 受阻中」，四项同排。
- F-5c 内置组件十六种，分「入口 / 暴露面」「内网资产」「网络 / 边界」「情报 / 战果」四组，不设「其它」。
- F-5b 链路不再有标签与攻击手法，只保留状态、备注与自定义字段。
- F-6a 渗透测试详情为富文本（段落、列表、粗体、内联图片），随报告预览与导出。
- F-6b 报告抬头按需列出「IP / 主机端口信息」与「域名端口信息」，地址没填的行不出现。
- F-7c 运行日志分两类：接口访问（时间、来源 IP、接口、请求值、状态码、返回值）与错误（时间、级别、来源 IP、接口、状态码、错误信息），管理员在顶栏「日志」查看。
- F-7d 日志里的口令与令牌字段一律脱敏为 ***。
- F-1c 项目带状态（进行中 / 已完成），新建与编辑共用同一表单，列表可直接编辑资料。
- F-6d 详情插图上传前压缩到长边 1600 再上传成独立资源；正文只存签名地址，单条拓扑的请求体远低于 48MB 上限。
- F-7a 用户管理支持建号、改角色、停用、重置口令与删除；最近登录按年月日时分显示。
- F-7b 画布提供「整理」，按依赖关系分层重排节点。
- F-33c 项目写入落文件日志：{DATA_DIR}/logs/projects.log，Tab 分隔 时间/项目/名称/操作人/动作/详情。
- F-30 版本查询只返回当前版本与仓库地址，不联网、不判断有没有新版本。
- F-31 界面不提供「检查更新 / 一键更新」；升级只有命令行一条路：`scripts/upgrade.sh`。
- F-32 升级脚本负责备份、取代码、构建、重启、健康检查，失败自动回滚。
- F-33 数据库迁移随二进制启动自动执行，升级无需手工建表。
- F-51 MCP 的 `set_vulnerability` 接受 Markdown，自动转成富文本 HTML（粗体、斜体、代码、有序/无序列表、图片），与界面编辑器的能力对齐。
- F-52 新增 MCP 工具 `upload_image`：base64 上传图片返回签名地址，Agent 可给渗透测试详情插图。
- F-53 详情编辑器支持 Ctrl+V 粘贴图片与拖拽图片，上传后自动插入。
- F-54 漏洞属性面板的富文本区有明确的「渗透测试详情」字段标签。
- F-55 组件库搜索走模糊匹配：子序列命中即算（`dbs` → 数据库，`dmn` → 域控 / AD），结果按相关度排序。
- F-56 Word 报告嵌入正文图片；取图失败会在控制台留痕，不静默丢图。

- F-41 画布顶栏「预览PNG」：先渲染出成品图并弹窗显示（带缩放与尺寸体积），满意再保存；「导出PNG」直接落盘。
- F-42 撤销栈最多 50 步，每次改图（新增/删除节点与链路、复制、反转、格式化）前压入快照；撤销后清空选择态。
- F-43 项目列表「最近更新」按年月日时分显示，悬停可看相对时间。
- F-44 审计面板支持按用户名与起止日期筛选；记录区固定高度内部滚动，「加载更早」固定在底部；面板高度不超过左栏。
- F-45 审计摘要统一为「用户名 + 动作 + 对象名 + 字段值」，例如「管理员 修改 马可波罗瓷砖官网 节点：组件类型 主机服务器 改为 Web 应用」。
- F-46 审计不比对漏洞说明、渗透详情与处置建议的正文，只在变化时记「已更新」，避免把图片数据写进记录。
- F-47 漏洞详情插入图片需要时随条目重建编辑器，新增漏洞时全部字段清空，不会残留上一条内容。
- F-48 正文插图走 `/api/pub/uploads/{id}?sig=`：HMAC 短签名代替登录态，图片可被浏览器长期缓存，签名错误与内容不存在都返回 404。
- F-49 项目列表带状态列：进行中用中性色块，已完成用绿色块。
- F-50 属性面板宽 420px，保证审计筛选器（用户名 10 字符 + 两个日期）完整可见。
## 4. 接口清单

统一前缀 `/api`，统一响应 `{ code, msg, data }`，HTTP 状态码与 code 一致。
**`POST /api/auth/login` 是唯一不需要鉴权的接口**，下表「权限」列标注「登录」的均需 Bearer 令牌。

| 编号 | 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- | --- |
| A-01 | POST | `/api/auth/login` | 公开 | 登录（唯一公开接口） |
| A-02 | POST | `/api/auth/logout` | 登录 | 登出（客户端丢弃令牌） |
| A-03 | GET | `/api/auth/me` | 登录 | 当前用户 |
| A-04 | POST | `/api/auth/password` | 登录 | 修改口令 |
| A-05 | GET | `/api/auth/users` | 登录 | 成员名录（仅 id/用户名/显示名） |
| S-01 | GET | `/api/system/version` | 登录 | 版本与仓库信息（含 Releases 查询地址） |
| H-01 | GET | `/api/health` | 登录 | 运维探针（版本与计数） |
| U-01 | GET | `/api/users` | 管理员 | 用户列表 |
| U-02 | POST | `/api/users` | 管理员 | 建号 |
| U-03 | PATCH | `/api/users/{id}` | 管理员 | 改角色/启停/重置口令 |
| P-01 | GET | `/api/projects` | 登录 | 项目列表（`?q=` 搜索） |
| P-02 | POST | `/api/projects` | 登录 | 新建项目 |
| P-03 | GET | `/api/projects/{id}` | 成员 | 项目详情 + 拓扑 |
| P-04 | PUT | `/api/projects/{id}` | editor | 保存拓扑（版本号校验） |
| P-05 | PATCH | `/api/projects/{id}` | owner | 改名称/描述 |
| P-06 | DELETE | `/api/projects/{id}` | owner | 删除项目 |
| P-07 | POST | `/api/projects/{id}/duplicate` | 成员 | 复制项目 |
| P-08 | GET | `/api/projects/{id}/export` | 成员 | 导出 bundle |
| P-09 | POST | `/api/projects/import` | 登录 | 导入 JSON 为新项目 |
| P-10 | GET | `/api/projects/{id}/members` | 成员 | 成员列表 |
| P-11 | POST | `/api/projects/{id}/members` | owner | 授权 |
| P-12 | DELETE | `/api/projects/{id}/members/{user_id}` | owner | 撤销授权 |
| P-13 | GET | `/api/projects/{id}/audit` | 成员 | 项目审计 |
| P-14 | GET | `/api/projects/{id}/presence` | 成员 | 在线成员快照 |
| P-15 | POST | `/api/projects/{id}/presence` | 成员 | 心跳 |
| T-01 | GET | `/api/asset-types` | 登录 | 组件类型清单 |
| T-02 | POST | `/api/asset-types` | 登录 | 新建自定义组件 |
| T-03 | PATCH | `/api/asset-types/{slug}` | 创建者/管理员 | 修改组件 |
| T-04 | DELETE | `/api/asset-types/{slug}` | 创建者/管理员 | 删除组件 |
| T-05 | POST | `/api/uploads` | 登录 | 上传图标（base64） |
| T-06 | GET | `/api/uploads/{id}` | 登录 | 读取图标（图片流） |
| L-01 | GET | `/api/audit` | 管理员 | 全局审计 |
| M-01 | POST | `/mcp` | Bearer 或 Basic | MCP JSON-RPC |

## 5. 非功能要求

- N-01 安全：全部 SQL 使用参数绑定；上传白名单 + 大小限制；无命令执行；错误信息不回传内部细节。
- N-02 并发：SQLite 开启 WAL，连接池上限 8；写操作单事务。
- N-03 可观测：请求日志（tracing），审计表记录业务动作。
- N-04 配置：密钥、数据库路径、端口等全部来自环境变量。
- N-05 升级：迁移随启动执行；版本检查只读本地清单，不发起外网请求。

## 6. 项目目录结构

```
redteam-topology-v3/
├─ doc/
│  ├─ srs.md               需求规格说明书
│  ├─ db.md                数据库设计与建表 SQL
│  ├─ openapi.md           REST 接口文档（OpenAPI 3.0）
│  └─ mcp.md               MCP 接入完整参考（面向 AI Agent）
├─ backend/
│  ├─ Cargo.toml
│  ├─ migrations/           sqlx 迁移脚本
│  └─ src/
│     ├─ main.rs            启动装配
│     ├─ config.rs          环境变量配置
│     ├─ error.rs           AppError → 统一响应
│     ├─ response.rs        ApiResponse 包装
│     ├─ state.rs           AppState
│     ├─ auth.rs            JWT 签发/校验与提取器
│     ├─ model/             DTO / Entity
│     ├─ db/                SQL 操作
│     ├─ service/           业务逻辑
│     ├─ handler/           入参校验与响应组装
│     ├─ router/            路由注册
│     └─ mcp/               MCP 工具实现
└─ frontend/
   ├─ .env.development / .env.production
   └─ src/
      ├─ types/             类型定义
      ├─ api/               axios 封装与接口
      ├─ stores/            Pinia
      ├─ views/             页面
      ├─ components/        公共组件
      ├─ router/            路由（懒加载）
      └─ styles/            Hallmark Console 设计系统
```

## 7. 实现说明（与设计的对应关系）

| 约定 | 落地方式 |
| --- | --- |
| 唯一公开接口 | 仅 `POST /api/auth/login`；`health`、`uploads/{id}` 也走 `AuthUser` 提取器 |
| 账号来源 | 管理员建号；首次启动按 `ATLAS_ADMIN_*` 创建管理员，之后不再自动建号 |
| 统一响应 | `ApiResponse{code,msg,data}`，HTTP 状态码与 code 一致；错误由 `AppError` 统一映射 |
| 401 语义 | 令牌缺失、过期、`token_version` 不匹配、账号停用，均返回 401 并提示重新登录 |
| 404 语义 | 无权访问的项目统一返回「项目不存在或无权访问」，避免探测项目是否存在 |
| 导出 | `GET /projects/{id}/export` 返回 `ExportBundle`（包在 `data` 内），前端生成 `.atlas.json` 下载 |
| 图标显示 | 图标接口需登录，浏览器无法给 `<img src>` 加头；前端带令牌取流转对象 URL 后渲染 |
| 在线协作 | presence 心跳 + 轮询（5s 心跳 / 6.5s 拉取），不引入长连接 |
| 自动保存 | 前端改动后 1.2s 防抖保存，携带 `baseVersion`；冲突返回 409 并给出两个选择 |
| 审计粒度 | 每次保存对比前后拓扑，生成 
ode.create/update/delete`、`edge.*`，`update` 附带字段级 diff |
| MCP 鉴权 | `Bearer` 走 JWT 校验；`Basic` 走用户名口令校验（不签发令牌，每次请求校验账号状态） |
| MCP 响应 | 保持 JSON-RPC 2.0 原格式（不套统一响应），工具错误以 `result.isError` 表达 |
| 版本查询 | 只读 `CARGO_PKG_VERSION` 与仓库地址，不联网、不判断新版本；升级走 `scripts/upgrade.sh` |

## 8. 变更记录

| 版本 | 内容 |
| --- | --- |
| v3.0.0 | 由 Node/Express 重构为 Rust Axum + sqlx；前端迁移 TypeScript；新增 JWT、MCP、组件上传、审计 |
| v3.1.0 | 关闭自助注册，仅登录公开，其余接口一律鉴权；MCP 支持 Basic 用户名口令；删除测试脚本与自检文档 |
| v3.1.1 | 文档：新增 doc/mcp.md（面向 Agent 的完整接入参考）；README 补 Windows 后台常驻、Docker 数据保存与 Docker 升级细则 |
| v3.1.2 | 文档：README 增「密钥与初始管理员」（ATLAS_JWT_SECRET / ATLAS_ADMIN_PASSWORD 默认行为与示例）与「更新迭代」（Windows / Linux / Docker 三环境迭代流程、迁移只增不改的约束） |
| v3.2.0 | 新增节点漏洞详情（双击打开，字段级审计）与 Word/PDF 报告导出；自定义组件支持 5 个内置图标；MCP 新增 `set_vulnerability`；`.env.example` 填默认值；Docker 数据改为宿主目录 `/data/atlas:/data` |
| v3.3.0 | 漏洞改为一个资产多条（列表 + 添加/删除）；报告增加预览页；MCP 增 `remove_vulnerability` 且 `set_vulnerability` 支持 `index`；新增「攻击者」组件；清理界面冗余操作说明 |