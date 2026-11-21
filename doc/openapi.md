# 接口文档（OpenAPI 3.0）

> 统一响应：`{ "code": 200, "msg": "ok", "data": any | null }`
> 鉴权：**仅 `POST /api/auth/login` 公开**；其余接口（含 health、uploads）都需要 `Authorization: Bearer <token>`
> 状态码：200 成功 / 400 参数错误 / 401 未登录 / 403 无权限 / 404 不存在 / 409 版本冲突 / 500 服务异常

```yaml
openapi: 3.0.3
info:
  title: RedTeam Atlas API
  version: 3.6.1
  description: 红队攻防资产与攻击路径拓扑平台
servers:
  - url: http://localhost:8080/api
    description: 本地开发
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
  schemas:
    ApiResponse:
      type: object
      properties:
        code: { type: integer, example: 200 }
        msg: { type: string, example: ok }
        data: { nullable: true }
    User:
      type: object
      properties:
        id: { type: string, example: "u_9f2c1a4b7d0e3f21" }
        username: { type: string, example: admin }
        displayName: { type: string, example: 管理员 }
        role: { type: string, enum: [admin, member] }
        disabled: { type: boolean }
        createdAt: { type: integer, example: 1763510000000 }
        lastLoginAt: { type: integer, nullable: true }
    LoginResult:
      type: object
      properties:
        token: { type: string, example: "eyJhbGciOiJIUzI1NiJ9..." }
        expiresAt: { type: integer, example: 1764114800000 }
        user: { $ref: '#/components/schemas/User' }
    GraphNode:
      type: object
      properties:
        id: { type: string, example: "n_1a2b3c4d5e6f7a8b" }
        position:
          type: object
          properties: { x: { type: number }, y: { type: number } }
        data:
          type: object
          properties:
            label: { type: string, example: "10.0.0.5" }
            kind: { type: string, example: host }
            ip: { type: string }
            port: { type: string }
            os: { type: string }
            status: { type: string, enum: [unknown, recon, vuln, owned, blocked] }
            tags: { type: array, items: { type: string } }
            notes: { type: string }
            vulns:
              type: array
              maxItems: 50
              description: 漏洞列表（双击节点编辑，可预览并导出 Word/PDF 报告）
              items:
                type: object
                properties:
                  id: { type: string, example: "v_1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d" }
                  name: { type: string, example: "任意文件上传漏洞" }
                  description: { type: string, example: "上传接口未校验文件类型" }
                  risk:
                    type: string
                    enum: [critical, high, medium, low, info, ""]
                    example: high
                  detail: { type: string, example: "1. 构造 multipart 请求…" }
                  remediation: { type: string, example: "白名单校验扩展名" }
                  images:
                    type: array
                    maxItems: 8
                    description: 详情配图（upload id 列表，透传 ./api/uploads 返回的 id）
                    items: { type: string, example: "img_1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d" }
                  updatedAt: { type: integer, nullable: true }
                  updatedBy: { type: string }
            fields:
              type: array
              items:
                type: object
                properties: { key: { type: string }, value: { type: string } }
    GraphEdge:
      type: object
      properties:
        id: { type: string }
        source: { type: string }
        target: { type: string }
        label: { type: string, example: Pass-the-Hash }
        data:
          type: object
          properties:
            technique: { type: string }
            status: { type: string, enum: [planned, active, success, failed] }
            notes: { type: string }
            fields: { type: array, items: { type: object } }
    Project:
      type: object
      properties:
        id: { type: string, example: "p_7c1e9a2b4d6f8031" }
        name: { type: string, example: "2026 某集团演练" }
        description: { type: string }
        ownerId: { type: string }
        version: { type: integer, example: 3 }
        nodeCount: { type: integer }
        edgeCount: { type: integer }
        permission: { type: string, enum: [owner, editor, viewer] }
        createdAt: { type: integer }
        updatedAt: { type: integer }
        updatedBy: { type: string }
    Graph:
      type: object
      properties:
        nodes: { type: array, items: { $ref: '#/components/schemas/GraphNode' } }
        edges: { type: array, items: { $ref: '#/components/schemas/GraphEdge' } }
        viewport:
          type: object
          properties: { x: { type: number }, y: { type: number }, zoom: { type: number } }
    AssetType:
      type: object
      properties:
        slug: { type: string, example: bastion }
        label: { type: string, example: 堡垒机 }
        color: { type: string, example: "#d9a441" }
        iconUrl: { type: string, nullable: true }
        builtin: { type: boolean }
        createdBy: { type: string, nullable: true }
    AuditRecord:
      type: object
      properties:
        id: { type: string }
        projectId: { type: string, nullable: true }
        userName: { type: string }
        action: { type: string, example: node.update }
        targetType: { type: string, example: node }
        targetId: { type: string }
        targetLabel: { type: string }
        summary: { type: string, example: "修改节点「WEB01」：状态 未知→已控制" }
        changes:
          type: array
          items:
            type: object
            properties:
              field: { type: string }
              from: { type: string }
              to: { type: string }
        createdAt: { type: integer }
  responses:
    Unauthorized:
      description: 未登录或令牌失效
      content:
        application/json:
          example: { code: 401, msg: 登录状态已失效，请重新登录, data: null }
    Forbidden:
      description: 无权限
      content:
        application/json:
          example: { code: 403, msg: 没有该项目的操作权限, data: null }
paths:
  /health:
    get:
      summary: 运维探针（需登录）
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data: { name: RedTeam Atlas, version: 3.1.0, users: 2, projects: 3, encrypted: true }
        '401': { $ref: '#/components/responses/Unauthorized' }

  /system/version:
    get:
      summary: 版本与仓库信息（前端据此查询 GitHub Releases）
      description: 返回当前版本、官网仓库与 Releases 查询地址；离线清单存在时一并给出对比结果。后端不访问外网，GitHub 查询由前端发起。
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  current: 3.1.0
                  latest: 3.2.0
                  updateAvailable: true
                  notes: "1. 新增攻击链路时间轴"
                  publishedAt: 1763500000000
                  source: manifest
                  repo: 1neptune/redteam-topology-v3
                  homepage: https://github.com/1neptune/redteam-topology-v3
                  releasesUrl: https://github.com/1neptune/redteam-topology-v3/releases
                  checkUrl: https://api.github.com/repos/1neptune/redteam-topology-v3/releases/latest
                  checkedAt: 1763600000000
                  upgradeHint:
                    - "Windows：停止 atlas.exe，替换为新的 redteam-atlas.exe，再启动"
                    - "Linux：systemctl stop atlas，覆盖 /usr/local/bin/redteam-atlas，systemctl start atlas"
                    - "Docker：docker compose pull && docker compose up -d"
        '401': { $ref: '#/components/responses/Unauthorized' }

  /system/logs:
    get:
      summary: 运行日志（管理员）
      description: kind=access 读接口访问日志，kind=error 读错误日志；返回最近 limit 条，最新的在最后
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: kind, in: query, required: false, schema: { type: string, enum: [access, error] } }
        - { name: limit, in: query, required: false, schema: { type: integer, default: 200, maximum: 1000 } }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  kind: access
                  columns: [at, ip, endpoint, request, status, response]
                  rows:
                    - ["2026-11-20 15:29:31", "127.0.0.1", "GET /api/projects", "", "200", "{\"code\":200}"]
        '403': { description: 非管理员 }
  /system/upgrade:
    post:
      summary: 一键更新（仅管理员）
      description: |
        访问 GitHub Releases 取最新版本，下载本平台的可执行文件到 `{数据目录}/upgrade/`，
        写下替换脚本并拉起，随后本进程退出；脚本等待退出后备份旧文件、替换、再启动新进程。

        容器部署直接拒绝，提示改用 `docker compose pull`。
        没有新版本、发布未提供对应平台的 asset、网络不可达都会返回 400 与具体原因。
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: 已下载并接管重启
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  from: 3.3.0
                  to: 3.4.0
                  asset: redteam-atlas-v3.4.0-x86_64-pc-windows-gnu.exe
                  staged: /srv/atlas/data/upgrade/redteam-atlas.new.exe
                  bytes: 6422528
                  script: /srv/atlas/data/upgrade/apply.ps1
                  restarting: true
        '400':
          description: 无新版本、无可用资产或网络不可达
          content:
            application/json:
              example: { code: 400, msg: 1neptune/redteam-topology-v3 仓库尚未发布正式版本, data: null }
        '401': { $ref: '#/components/responses/Unauthorized' }

  /auth/login:
    post:
      summary: 登录
      security: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [username, password]
              properties:
                username: { type: string }
                password: { type: string }
      responses:
        '200':
          description: 登录成功
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  token: "eyJhbGciOiJIUzI1NiJ9..."
                  expiresAt: 1764114800000
                  user: { id: u_1, username: admin, displayName: 管理员, role: admin, disabled: false }
        '401': { $ref: '#/components/responses/Unauthorized' }

  /auth/logout:
    post:
      summary: 登出（客户端丢弃令牌）
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example: { code: 200, msg: ok, data: null }

  /auth/me:
    get:
      summary: 当前用户
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example: { code: 200, msg: ok, data: { id: u_1, username: admin, displayName: 管理员, role: admin } }
        '401': { $ref: '#/components/responses/Unauthorized' }

  /auth/password:
    post:
      summary: 修改口令（旧令牌立即失效）
      security: [{ bearerAuth: [] }]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [current, next]
              properties:
                current: { type: string }
                next: { type: string, minLength: 6 }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example: { code: 200, msg: ok, data: { token: "eyJ...", expiresAt: 1764114800000 } }

  /auth/users:
    get:
      summary: 成员名录（供授权选择）
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data: [{ id: u_1, username: admin, displayName: 管理员 }]

  /users:
    get:
      summary: 用户列表（管理员）
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data: [{ id: u_1, username: admin, displayName: 管理员, role: admin, disabled: false }]
    post:
      summary: 建号（管理员）
      security: [{ bearerAuth: [] }]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [username, password]
              properties:
                username: { type: string }
                password: { type: string }
                displayName: { type: string }
                role: { type: string, enum: [admin, member] }
      responses:
        '200':
          description: 创建成功

  /users/{id}:
    delete:
      summary: 删除账号（管理员）
      description: 不能删除自己，也不能删掉最后一个管理员；同时移除其在所有项目中的授权
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200':
          description: 已删除
          content:
            application/json:
              example: { code: 200, msg: ok, data: { deleted: u_9f2c1a4b7d0e3f21 } }
        '400': { description: 删除自己或最后一个管理员时拒绝 }
    patch:
      summary: 改角色 / 启停 / 重置口令（管理员）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                displayName: { type: string }
                role: { type: string, enum: [admin, member] }
                disabled: { type: boolean }
                password: { type: string, minLength: 6 }
      responses:
        '200':
          description: OK

  /projects:
    get:
      summary: 项目列表（按权限过滤，支持搜索）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: q, in: query, required: false, schema: { type: string }, description: 名称/描述关键字 }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  - { id: p_1, name: 2026 某集团演练, status: active, nodeCount: 12, edgeCount: 11, permission: owner, version: 3 }
    post:
      summary: 新建项目
      security: [{ bearerAuth: [] }]
      requestBody:
        content:
          application/json:
            schema:
              type: object
              required: [name]
              properties:
                name: { type: string }
                description: { type: string }
                data: { $ref: '#/components/schemas/Graph' }
      responses:
        '200':
          description: 创建成功

  /projects/{id}:
    get:
      summary: 项目详情（含解密后的拓扑）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  project: { id: p_1, name: 演练, version: 3, permission: owner }
                  permission: owner
                  graph:
                    nodes: [{ id: n_1, position: { x: 100, y: 100 }, data: { label: WEB01, kind: host } }]
                    edges: []
    put:
      summary: 保存拓扑（版本号乐观锁）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                data: { $ref: '#/components/schemas/Graph' }
                baseVersion: { type: integer, description: 缺省则不校验版本 }
      responses:
        '200':
          description: 保存成功
          content:
            application/json:
              example: { code: 200, msg: ok, data: { project: { version: 4 }, changes: 3 } }
        '409':
          description: 版本冲突
          content:
            application/json:
              example: { code: 409, msg: 项目已被其他成员更新, data: { version: 5, updatedBy: 李四 } }
    patch:
      summary: 修改项目资料（owner）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                name: { type: string }
                description: { type: string }
      responses:
        '200': { description: OK }
    delete:
      summary: 删除项目（owner）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200': { description: OK }

  /projects/{id}/duplicate:
    post:
      summary: 复制项目
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200': { description: OK }

  /projects/{id}/export:
    get:
      summary: 导出 .atlas.json
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200':
          description: JSON 文件
          content:
            application/json:
              example:
                format: redteam-atlas
                schemaVersion: 3
                project: { name: 演练, description: '', data: { nodes: [], edges: [] } }

  /projects/import:
    post:
      summary: 导入 JSON 为新项目
      security: [{ bearerAuth: [] }]
      requestBody:
        required: true
        content:
          application/json:
            example:
              project:
                name: 导入的演练
                description: 来自 ARTEX
                data: { nodes: [], edges: [] }
      responses:
        '200': { description: 导入成功 }

  /projects/{id}/members:
    get:
      summary: 成员列表
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  - { userId: u_2, displayName: 李四, permission: editor, grantedAt: 1763510000000 }
    post:
      summary: 授权（owner 或管理员）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [userId, permission]
              properties:
                userId: { type: string }
                permission: { type: string, enum: [viewer, editor, owner] }
      responses:
        '200': { description: OK }

  /projects/{id}/members/{userId}:
    delete:
      summary: 撤销授权
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
        - { name: userId, in: path, required: true, schema: { type: string } }
      responses:
        '200': { description: OK }

  /projects/{id}/audit:
    get:
      summary: 项目审计轨迹
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
        - { name: limit, in: query, schema: { type: integer, default: 40 } }
        - { name: before, in: query, schema: { type: integer }, description: 上一页最后一条时间戳 }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  - id: a_1
                    userName: 管理员
                    action: node.update
                    targetType: node
                    targetLabel: WEB01
                    summary: "修改节点「WEB01」：状态 未知→已控制"
                    changes: [{ field: 状态, from: 未知, to: 已控制 }]
                    createdAt: 1763510000000

  /projects/{id}/presence:
    get:
      summary: 在线成员快照（15 秒内有心跳）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data: [{ userId: u_2, userName: 李四, cursorX: 320, cursorY: 180, seenAt: 1763510000000 }]
    post:
      summary: 心跳 + 光标
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                cursorX: { type: number }
                cursorY: { type: number }
      responses:
        '200': { description: OK }

  /asset-types:
    get:
      summary: 组件类型（内置 + 自定义）
      security: [{ bearerAuth: [] }]
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                code: 200
                msg: ok
                data:
                  - { slug: host, label: 主机服务器, color: "#9fb3c8", builtin: true }
                  - { slug: ct-1a2b3c4d, label: 堡垒机, color: "#d9a441", iconUrl: /api/uploads/img_1, builtin: false }
    post:
      summary: 新建自定义组件
      security: [{ bearerAuth: [] }]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [label]
              properties:
                label: { type: string, maxLength: 24 }
                color: { type: string, example: "#d9a441" }
                iconUrl: { type: string }
      responses:
        '200': { description: OK }

  /asset-types/{slug}:
    patch:
      summary: 修改自定义组件（创建者或管理员）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: slug, in: path, required: true, schema: { type: string } }
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                label: { type: string }
                color: { type: string }
                iconUrl: { type: string }
      responses:
        '200': { description: OK }
    delete:
      summary: 删除自定义组件
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: slug, in: path, required: true, schema: { type: string } }
      responses:
        '200': { description: OK }

  /uploads:
    post:
      summary: 上传图标（base64，≤2MB，仅图片）
      security: [{ bearerAuth: [] }]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [filename, mime, dataBase64]
              properties:
                filename: { type: string }
                mime: { type: string, enum: [image/png, image/jpeg, image/webp, image/gif, image/svg+xml] }
                dataBase64: { type: string }
      responses:
        '200':
          description: OK
          content:
            application/json:
              example: { code: 200, msg: ok, data: { id: img_1, url: /api/uploads/img_1, bytes: 1024, mime: image/png } }

  /pub/uploads/{id}:
    get:
      summary: 读取正文插图（公开，凭短签名）
      description: 富文本正文里图片用这个地址，签名由 HMAC(master_key, id) 生成，不随登录状态变化；签名错误与不存在都返回 404
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
        - { name: sig, in: query, required: true, schema: { type: string }, description: 32 位十六进制签名 }
      responses:
        '200':
          description: 图片二进制，带 immutable 长缓存
          content:
            image/png:
              schema: { type: string, format: binary }
        '404': { description: 签名无效或内容不存在 }
  /uploads/{id}:
    get:
      summary: 读取图标（需登录，返回图片流）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '200':
          description: 图片二进制
          content:
            image/png:
              schema: { type: string, format: binary }

  /audit:
    get:
      summary: 全局审计（管理员）
      security: [{ bearerAuth: [] }]
      parameters:
        - { name: limit, in: query, schema: { type: integer, default: 80 } }
      responses:
        '200': { description: OK }
```

## MCP 端点（非 REST，保持 JSON-RPC 原格式）

`POST /mcp`，请求体为 JSON-RPC 2.0。鉴权支持两种请求头：

| 方式 | 请求头 |
| --- | --- |
| JWT | `Authorization: Bearer <token>` |
| 用户名口令 | `Authorization: Basic base64(用户名:口令)` |

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "ingest_attack_path",
    "arguments": {
      "project_name": "2026 某集团演练",
      "nodes": [
        { "id": "a1", "label": "外网门户", "kind": "entry", "ip": "198.51.100.7", "status": "vuln" },
        { "id": "a2", "label": "域控 DC01", "kind": "domain", "ip": "10.1.1.10", "status": "owned" }
      ],
      "edges": [{ "from": "a1", "to": "a2", "label": "SSRF 打点", "technique": "SSRF", "status": "success" }]
    }
  }
}
```

响应：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{ "type": "text", "text": "{\"project_id\":\"p_1\",\"added_nodes\":2,\"added_edges\":1}" }],
    "isError": false
  }
}
```

## 错误响应样例

| 场景 | HTTP | 响应体 |
| --- | --- | --- |
| 参数缺失 | 400 | `{"code":400,"msg":"用户名不能为空","data":null}` |
| 未登录 | 401 | `{"code":401,"msg":"登录状态已失效，请重新登录","data":null}` |
| 无权限 | 403 | `{"code":403,"msg":"没有该项目的操作权限","data":null}` |
| 不存在/无权访问 | 404 | `{"code":404,"msg":"项目不存在或无权访问","data":null}` |
| 版本冲突 | 409 | `{"code":409,"msg":"项目已被其他成员更新","data":{"version":5}}` |
| 服务异常 | 500 | `{"code":500,"msg":"服务器内部错误","data":null}` |