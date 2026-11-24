# MCP 接入说明

给 AI Agent 与平台集成方的完整参考。人类用户的快速上手在 [README](../README.md#mcp-接入)。

RedTeam Atlas 通过 MCP（Model Context Protocol）把渗透测试结果落成攻击路径拓扑：
一个 `ingest_attack_path` 调用就能画完一条从外网打点到域控接管的链路，并在界面上留下审计记录。

---

## 1. 端点

| 项 | 值 |
| --- | --- |
| URL | `http://<host>:8080/mcp` |
| 方法 | `POST`（`GET` 返回 405） |
| Content-Type | `application/json` |
| 协议 | JSON-RPC 2.0 |
| 传输 | Streamable HTTP（无 SSE、无会话保持，每次请求独立） |
| 协议版本 | `2024-11-05`、`2025-03-26`、`2025-06-18`（按客户端请求回选，未知版本回落到最新） |
| 响应格式 | 保持 JSON-RPC 原格式，**不套** `{code,msg,data}` |

服务端标识：`serverInfo = { name: "redteam-atlas", version: "<后端版本>" }`。

---

## 2. 鉴权

两种方式等价，任选其一，都放在 `Authorization` 头。

### 2.1 Bearer 令牌

```
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...
```

令牌来自登录接口，默认 7 天有效：

```bash
curl -s http://<host>:8080/api/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"<口令>"}'
```

响应 `data.token` 即为令牌。

### 2.2 HTTP Basic 用户名口令

```
Authorization: Basic YWRtaW46cGFzc3dvcmQ=
```

内容是 `base64(用户名:口令)`：

```bash
# Linux / macOS
printf 'admin:<口令>' | base64 -w0
```

```powershell
# Windows PowerShell
[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('admin:<口令>'))
```

Basic 每次请求都会校验账号状态与口令，因此改密或停用会立即作用于 Agent，无需等令牌过期。
口令以 base64 明文传输，**必须走 HTTPS**。

### 2.3 客户端配置

```json
{
  "mcpServers": {
    "redteam-atlas": {
      "url": "http://<host>:8080/mcp",
      "headers": { "Authorization": "Basic YWRtaW46cGFzc3dvcmQ=" }
    }
  }
}
```

把 `headers.Authorization` 换成 `Bearer <令牌>` 即为 JWT 方式。

### 2.4 鉴权失败

| 情况 | HTTP | 响应体 |
| --- | --- | --- |
| 未带 `Authorization` | 401 | `{"code":401,"msg":"请先登录","data":null}` |
| Basic 口令错误 | 401 | `{"code":401,"msg":"用户名或口令错误","data":null}` |
| 账号已停用 | 403 | `{"code":403,"msg":"账号已被停用","data":null}` |
| 令牌过期或改密 | 401 | `{"code":401,"msg":"登录状态已失效，请重新登录","data":null}` |

鉴权失败发生在 JSON-RPC 之前，返回的**不是** JSON-RPC 错误对象，而是平台的统一响应结构。

---

## 3. 握手

标准三步。服务端无状态，`initialize` 不是必需的，但建议按序执行。

### 3.1 initialize

```bash
curl -s http://<host>:8080/mcp \
  -H "Authorization: Basic <base64>" \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}'
```

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": { "tools": { "listChanged": false } },
    "serverInfo": { "name": "redteam-atlas", "version": "4.4.1" }
  }
}
```

### 3.2 notifications/initialized

```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

任何 `notifications/*` 方法都返回 **202 无响应体**。

### 3.3 tools/list

```bash
curl -s http://<host>:8080/mcp \
  -H "Authorization: Bearer <令牌>" \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
```

返回 20 个工具的完整 JSON Schema。

### 3.4 调用工具

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": { "name": "<工具名>", "arguments": { } }
}
```

成功响应：

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [{ "type": "text", "text": "{\"count\":2,...}" }],
    "isError": false
  }
}
```

**`result.content[0].text` 是字符串**，里面才是工具返回的 JSON。Agent 需要二次解析：

```python
payload = json.loads(response["result"]["content"][0]["text"])
```

工具业务失败时 `isError` 为 `true`，`text` 直接是中文错误消息（不是 JSON）：

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": { "content": [{ "type": "text", "text": "项目不存在或无权访问" }], "isError": true }
}
```

---

## 4. 工具参考

共 20 个工具。权限按用途分三档：

- 读取类（`list_projects`、`get_project`、`search_graph`、`list_asset_types`）需对该项目有 viewer 及以上权限；
- 写入类（`ingest_attack_path`、`add_nodes`、`add_edges`、`update_node`、`set_vulnerability`、`remove_vulnerability`、`delete_node`、`delete_edge`、`upload_image`）需 editor 及以上；
- 管理类（`update_project`、`delete_project`、`create_asset_type`、`add_member`）需项目 owner，`list_users`、`create_user` 需系统管理员。

管理员对所有项目具备全部权限。

### 4.1 list_projects

列出当前凭据可见的项目。

```json
{ "name": "list_projects", "arguments": {} }
```

返回数组：

```json
[
  {
    "id": "p_9f2c1a4b7d0e3f21",
    "name": "2026 某集团演练",
    "nodeCount": 12,
    "edgeCount": 11,
    "version": 3,
    "permission": "owner",
    "updatedAt": 1763510000000,
    "updatedBy": "管理员"
  }
]
```

`permission` 取值：`owner`、`editor`、`viewer`。只读项目不要尝试写入。

### 4.2 get_project

读取完整拓扑，用于在已有图上做增量修改。

```json
{ "name": "get_project", "arguments": { "project_id": "p_9f2c..." } }
```

返回：

```json
{
  "project": { "id": "p_9f2c...", "name": "2026 某集团演练", "version": 3, "nodeCount": 12, "edgeCount": 11 },
  "nodes": [
    {
      "id": "n_1a2b...",
      "type": "asset",
      "position": { "x": 100, "y": 100 },
      "data": {
        "label": "外网门户",
        "kind": "entry",
        "ip": "198.51.100.7",
        "port": "443",
        "os": "",
        "status": "attack",
        "notes": "",
        "fields": [{ "key": "FOFA", "value": "body=\"Portal\"" }]
      }
    }
  ],
  "edges": [
    {
      "id": "e_a1b2...",
      "source": "n_1a2b...",
      "target": "n_3c4d...",
      "label": "SSRF 打点",
      "data": { "status": "success", "notes": "", "fields": [], "animated": false }
    }
  ]
}
```

### 4.3 create_project

新建空项目，返回项目摘要（结构同 `list_projects` 的单项）。

```json
{
  "name": "create_project",
  "arguments": { "name": "2026 某集团演练", "description": "目标范围与时间窗口" }
}
```

`name` 必填，服务端会去空白并截断到 80 字符。

### 4.4 ingest_attack_path

一次写入一批节点与链路。核心工具，优先用它。

```json
{
  "name": "ingest_attack_path",
  "arguments": {
    "project_name": "2026 某集团演练",
    "nodes": [
      { "id": "a1", "label": "外网门户", "kind": "entry",  "ip": "198.51.100.7", "port": "443", "status": "attack" },
      { "id": "a2", "label": "堡垒机",   "kind": "pivot",  "ip": "10.1.1.5",     "status": "control" },
      { "id": "a3", "label": "域控 DC01", "kind": "domain", "ip": "10.1.1.10",    "status": "control" }
    ],
    "edges": [
      { "from": "a1", "to": "a2", "label": "SSRF 打点",          "status": "success" },
      { "from": "a2", "to": "a3", "label": "PtH 横向", "status": "success" }
    ]
  }
}
```

参数：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `project_id` | string | 二选一 | 写入已有项目 |
| `project_name` | string | 二选一 | 按名称查找可写项目；不存在则自动创建 |
| `nodes` | array | 至少一个非空 | 见下 |
| `edges` | array | 同上 | 见下 |

`nodes[]`：

| 字段 | 类型 | 必填 | 默认 | 说明 |
| --- | --- | --- | --- | --- |
| `label` | string | 是 | — | 节点显示名，也是去重键 |
| `id` | string | 否 | — | 你方外部 id，用于 `edges.from/to` 引用与 `id_map` 回传 |
| `kind` | string | 否 | `host` | 组件类型，见 §6.1 |
| `ip` | string | 否 | 空 | IP 或主机名 |
| `port` | string | 否 | 空 | 端口 |
| `domain` | string | 否 | 空 | 域名，可写成 `www.a.com:443` |
| `os` | string | 否 | 空 | 操作系统或环境 |
| `status` | string | 否 | `recon` | 见 §6.2 |
| `notes` | string | 否 | 空 | 备注 |

`edges[]`：

| 字段 | 类型 | 必填 | 默认 | 说明 |
| --- | --- | --- | --- | --- |
| `from` | string | 是 | — | 源节点，可填外部 `id` 或节点 `label` |
| `to` | string | 是 | — | 目标节点，同上 |
| `label` | string | 否 | 空 | 链路上显示的文字 |
| `notes` | string | 否 | 空 | 利用条件与所需凭据 |
| `status` | string | 否 | `planned` | 见 §6.2 |

返回：

```json
{
  "project_id": "p_9f2c...",
  "version": 2,
  "added_nodes": 3,
  "added_edges": 2,
  "node_total": 3,
  "edge_total": 2,
  "id_map": { "a1": "n_7f3a...", "a2": "n_8b4c...", "a3": "n_9d5e..." },
  "editor_path": "/#/p/p_9f2c..."
}
```

`editor_path` 拼上域名即可点开：`http://<host>:8080/#/p/p_9f2c...`。把它回给用户是最省事的交付。

### 4.5 add_nodes

向已有项目追加节点，参数与返回同 `ingest_attack_path`（`edges` 为空）。

```json
{
  "name": "add_nodes",
  "arguments": {
    "project_id": "p_9f2c...",
    "nodes": [{ "label": "新发现的跳板机", "kind": "pivot", "ip": "10.1.1.30", "status": "control" }]
  }
}
```

### 4.6 add_edges

向已有项目追加链路。`from`/`to` 接受节点 `id` 或 `label`。

```json
{
  "name": "add_edges",
  "arguments": {
    "project_id": "p_9f2c...",
    "edges": [{ "from": "堡垒机", "to": "域控 DC01", "label": "PtH 横向", "status": "success" }]
  }
}
```

### 4.7 update_node

改单个节点的字段，`patch` 中只传要改的键。

```json
{
  "name": "update_node",
  "arguments": {
    "project_id": "p_9f2c...",
    "node_id": "n_7f3a...",
    "patch": {
      "status": "control",
      "domain": "pmp.example.com",
      "notes": "已拿到 SYSTEM 权限",
      "fields": [
        { "key": "后台管理员账号", "value": "admin_pmp" },
        { "key": "SSL证书到期日", "value": "2027-03-15" }
      ]
    }
  }
}
```

`node_id` 也接受节点 `label`。可改字段：

| 键 | 类型 | 说明 |
| --- | --- | --- |
| `label` | string | 节点名，画布上最多两行，完整内容在悬停提示里 |
| `kind` | string | 组件类型 slug |
| `ip` / `port` / `domain` | string | 地址；`ip` 与 `domain` 可只填一个，`port` 会拼在两者后面 |
| `os` | string | 操作系统 |
| `status` | string | `recon` / `attack` / `control` / `blocked` |
| `notes` | string | 备注 |
| `fields` | array | 自定义属性，`[{key, value}]`。**整组替换**，传 `[]` 清空，不传则保持原样 |

自定义属性就是画布节点卡片下半部分那几行，字段名与值都会完整换行显示，不会被截断。

返回：

```json
{ "version": 4, "actions": 1 }
```

`actions` 是本次产生的审计条数。

### 4.8 set_vulnerability

写入节点的渗透测试漏洞详情。这些字段就是导出 Word / PDF 报告的内容来源，
界面上对应「双击节点」打开的抽屉。

```json
{
  "name": "set_vulnerability",
  "arguments": {
    "project_id": "p_9f2c...",
    "node_id": "外网门户",
    "index": 0,
    "name": "任意文件上传漏洞",
    "description": "上传接口未校验文件类型与后缀，可上传 JSP 木马并直接访问执行。",
    "risk": "high",
    "detail": "1. POST /upload，multipart 上传 shell.jsp\n2. 访问该路径回显 uid=0(root)\n\n![回显截图](/api/pub/uploads/img_xxx?sig=xxx)",
    "remediation": "1. 白名单校验扩展名与 MIME\n2. 上传目录禁止脚本解析\n3. 文件重命名并存放于独立域名"
  }
}
```

`detail` 支持两种写法，**推荐 Markdown**：

| 写法 | 说明 |
| --- | --- |
| Markdown | 后端自动转成 HTML。支持 `**粗体**`、`*斜体*`、`` `代码` ``、`- 无序列`、`1. 有序列`、`![说明](图片地址)` |
| HTML | 以 `<` 开头时原样保留，可用 `<p>` `<b>` `<i>` `<ul>` `<ol>` `<li>` `<img>` `<code>` |

插图要先用 [`upload_image`](#420-upload_image) 上传拿到地址，再写进 `![说明](地址)`。

参数（`project_id`、`node_id` 之外都可选，只传要写的字段）：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `project_id` | string | 必填 |
| `node_id` | string | 必填。填节点 `label` 或 `get_project` 返回的内部 `id`；**不是** ingest 时的外部 id（`a1` 这类只在当次 `edges` 里有效） |
| `index` | integer | 可选。第几条漏洞（从 0 开始）；省略则**追加**一条新的，给了就覆盖那一条 |
| `name` | string | 漏洞名称，≤120 字符 |
| `description` | string | 漏洞说明，≤8000 字符 |
| `risk` | string | `critical`/`high`/`medium`/`low`/`info`，其它值会被清空 |
| `detail` | string | 渗透测试详情，富文本 HTML（`<p>` `<b>` `<ul>` `<img>` 等），≤400 万字符 |
| `remediation` | string | 处置建议，≤8000 字符 |

返回：

```json
{
  "version": 5,
  "actions": 3,
  "node_id": "a1",
  "risk": "high",
  "filled": true,
  "editor_path": "/#/p/p_9f2c..."
}
```

`actions` 是本次产生的审计条数：改动 3 个字段就是 3 条；只传 `risk` 时是 1 条。
写入会覆盖同名节点的既有漏洞详情；只传部分字段时其余字段保持不变。

### 4.9 remove_vulnerability

删除节点上的一条漏洞记录。

```json
{
  "name": "remove_vulnerability",
  "arguments": { "project_id": "p_9f2c...", "node_id": "外网门户", "index": 1 }
}
```

```json
{ "version": 6, "actions": 1, "removed": "SQL 注入", "total": 1, "editor_path": "/#/p/p_9f2c..." }
```

`index` 从 0 开始，越界返回 `漏洞记录不存在`。删除同样产生审计。

### 4.10 search_graph

在项目内按关键字搜索节点，匹配 `label`、`ip`、`os`、`notes`（不区分大小写）。

```json
{ "name": "search_graph", "arguments": { "project_id": "p_9f2c...", "keyword": "10.1.1" } }
```

```json
{
  "count": 2,
  "nodes": [
    { "id": "n_8b4c...", "label": "堡垒机", "kind": "pivot", "ip": "10.1.1.5", "status": "control" },
    { "id": "n_9d5e...", "label": "域控 DC01", "kind": "domain", "ip": "10.1.1.10", "status": "control" }
  ]
}
```

### 4.11 list_asset_types

可选组件类型，用于给 `nodes[].kind` 选值。

```json
{ "name": "list_asset_types", "arguments": {} }
```

```json
[
  { "slug": "entry", "label": "攻击入口", "builtin": true },
  { "slug": "ct-3ea80920", "label": "堡垒机", "builtin": false }
]
```

### 4.12 update_project

改项目名称、说明或状态，需要项目 owner 权限。`status` 只认 `active`（进行中）与 `done`（已完成），其它值按进行中处理。

```json
{ "name": "update_project", "arguments": { "project_id": "p_9a2f", "status": "done" } }
```

```json
{ "id": "p_9a2f", "name": "2026 某集团演练", "status": "done", "nodeCount": 12, "edgeCount": 11, "permission": "owner", "version": 7 }
```

### 4.13 delete_project

删除项目及其拓扑，需要 owner 权限。

```json
{ "name": "delete_project", "arguments": { "project_id": "p_9a2f" } }
```

```json
{ "deleted": "p_9a2f" }
```

### 4.14 delete_node

删节点并连带删除它的所有链路。`node_id` 可以填内部 id，也可以直接填节点名称。

```json
{ "name": "delete_node", "arguments": { "project_id": "p_9a2f", "node_id": "堡垒机" } }
```

```json
{ "version": 8, "nodes": 11, "edges": 9, "editor_path": "/#/p/p_9a2f" }
```

### 4.15 delete_edge

按链路 id 删除一条链路。

```json
{ "name": "delete_edge", "arguments": { "project_id": "p_9a2f", "edge_id": "e_7c1d" } }
```

```json
{ "version": 9, "nodes": 11, "edges": 8, "editor_path": "/#/p/p_9a2f" }
```

### 4.16 create_asset_type

新建自定义组件类型，创建后 `nodes[].kind` 就能用它。

```json
{ "name": "create_asset_type", "arguments": { "label": "堡垒机", "color": "#c9a87f", "icon_url": "builtin:pivot" } }
```

```json
{ "slug": "ct-3ea80920", "label": "堡垒机", "iconUrl": "builtin:pivot" }
```

### 4.17 add_member

把已有账号授权到项目，需要项目 owner 权限。`permission` 取 `owner` / `editor` / `viewer`，默认 `editor`。

```json
{ "name": "add_member", "arguments": { "project_id": "p_9a2f", "username": "zhangsan", "permission": "editor" } }
```

```json
[
  { "userId": "u_9f2c", "username": "admin", "permission": "owner" },
  { "userId": "u_3b7d", "username": "zhangsan", "permission": "editor" }
]
```

### 4.18 list_users

列出全部账号，需要系统管理员。口令不在返回里。

```json
{ "name": "list_users", "arguments": {} }
```

```json
[{ "id": "u_9f2c", "username": "admin", "displayName": "管理员", "role": "admin", "disabled": false }]
```

### 4.19 create_user

新建账号，需要系统管理员。口令至少 8 位且同时含大写与小写；给 `project_id` 就顺带授权，一个调用完成建号与授权。

```json
{
  "name": "create_user",
  "arguments": {
    "username": "zhangsan",
    "password": "Atlas#2026",
    "display_name": "张三",
    "role": "member",
    "project_id": "p_9a2f",
    "permission": "editor"
  }
}
```

```json
{ "id": "u_3b7d", "username": "zhangsan", "displayName": "张三", "role": "member" }
```

---

### 4.20 upload_image

上传一张图片，返回带签名的短地址。给 `set_vulnerability` 的 `detail` 插图用。

图片落在数据目录的 `uploads/`，地址形如 `/api/pub/uploads/img_xxx?sig=xxx`——签名随附件 id 生成，
不带签名访问返回 404，所以这个地址可以直接写进详情与报告，不必额外鉴权。

```json
{
  "name": "upload_image",
  "arguments": {
    "filename": "sqlmap-output.png",
    "mime": "image/png",
    "data_base64": "iVBORw0KGgoAAAANSUhEUg..."
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `data_base64` | string | 是 | 图片的 base64，不带 `data:` 前缀 |
| `mime` | string | 否 | `image/png`（默认）、`image/jpeg`、`image/webp`、`image/gif`、`image/svg+xml` |
| `filename` | string | 否 | 仅用于展示 |

返回：

```json
{
  "id": "img_8e96ed2129574f61a154ba78fe6fb5d3",
  "url": "/api/pub/uploads/img_8e96ed2129574f61a154ba78fe6fb5d3?sig=918cf100eb1c0...",
  "bytes": 48213,
  "mime": "image/png",
  "usage": "<img src=\"/api/pub/uploads/img_8e96ed2129574f61a154ba78fe6fb5d3?sig=918cf100eb1c0...\">"
}
```

`usage` 是可以直接粘进 HTML 写法的片段；用 Markdown 写法则拼成 `![说明](url)`。

**完整流程**

```json
// 1. 上传截图
{"name": "upload_image", "arguments": {"mime": "image/png", "data_base64": "iVBOR..."}}
// → url: /api/pub/uploads/img_abc?sig=def

// 2. 把地址写进漏洞详情
{"name": "set_vulnerability", "arguments": {
  "project_id": "p_9f2c...", "node_id": "外网门户",
  "name": "SQL 注入", "risk": "high",
  "detail": "复现步骤\n\n1. 访问 /search?q=1\n2. 观察报错\n\n**可利用**，影响 *全站数据*。\n\n![报错页面](/api/pub/uploads/img_abc?sig=def)"
}}
```

单张上限 2MB，超出直接报错。图片存进数据目录后不会自动清理，删库不删图。
## 5. 典型工作流

### 5.1 从扫描结果画一张新图

1. `list_projects` 看是否已有同名项目
2. `ingest_attack_path` 带 `project_name` 一次写完
3. 把返回的 `editor_path` 回给用户

```text
tools/call ingest_attack_path { project_name: "<目标演练名>", nodes: [...], edges: [...] }
→ editor_path: /#/p/p_xxx
```

### 5.2 在已有图上补一环

1. `search_graph` 找到父节点 id
2. `add_nodes` 加新节点，从 `id_map` 拿到新 id
3. `add_edges` 连起来

### 5.3 回填战果

渗透推进后更新状态：

```text
tools/call update_node { project_id, node_id, patch: { status: "control", notes: "..." } }
```

每次 `update_node` 都会在界面的「审计记录」里留下一条带字段 diff 的记录。

### 5.4 读整张图再决定

```text
tools/call get_project { project_id }
```

`nodes[].data.label` 适合直接展示给用户；`nodes[].data.status` 四档：recon / attack / control / blocked。

---

## 6. 取值约定

### 6.1 组件类型 kind

内置 14 个：

| slug | 名称 | slug | 名称 |
| --- | --- | --- | --- |
| `entry` | 攻击入口 | `network` | 网络 / 网段 |
| `web` | Web 应用 | `firewall` | 防火墙 / WAF |
| `cloud` | 云资源 | `pivot` | 跳板 / 隧道 |
| `host` | 主机服务器 | `credential` | 凭证 / 账号 |
| `domain` | 域控 / AD | `vuln` | 漏洞利用点（组件名，非状态） |
| `database` | 数据库 | `flag` | 战果 / 目标 |
| `workstation` | 工作站 | `note` | 备注 / 分组 |

管理员或组员在界面里创建的自定义组件，slug 形如 `ct-3ea80920`，先用 `list_asset_types` 查再填。
未知 slug 不会报错，界面会按 `host` 的外观渲染。

### 6.2 状态

节点 `status`：

节点 `status` 四档：

| 值 | 含义 |
| --- | --- |
| `recon` | 侦察中（默认） |
| `attack` | 攻击中 |
| `control` | 已控制 |
| `blocked` | 受阻中 |

链路 `status`：

| 值 | 含义 |
| --- | --- |
| `planned` | 计划中（默认，界面画成虚线） |
| `active` | 进行中 |
| `success` | 已成功 |
| `failed` | 失败 |

非法值会被后端规整为默认值，不报错。

### 6.4 文本限制

| 字段 | 上限 |
| --- | --- |
| 节点 label | 80 字符 |
| ip / os | 64 / 80 字符 |
| notes | 4000 字符 |
| 链路 label | 80 字符 |
| 单项目节点数 | 20000 |
| 单项目链路数 | 40000 |
| 单次 ingest 节点数 | 500 |
| 单次 ingest 链路数 | 1000 |

---

## 7. ingest_attack_path 的语义

### 7.1 项目定位

按顺序：

1. 给了 `project_id`：直接读该项目，校验写权限
2. 给了 `project_name`：在可见项目中找同名且非 viewer 的，找到就写入
3. 都没有或没找到：以 `project_name`（缺省「AI 生成的项目」）新建，描述写「由 AI Agent 通过 MCP 自动生成」

同一个 Agent 反复调用同一 `project_name` 不会造出一堆同名项目。

### 7.2 节点去重

按 `label` 完全相同去重：已存在同名节点时复用，不新建，且把外部 `id` 映射到已有节点 id。
因此**label 是业务主键**，同一项目里不要给两台不同的机器取一样的名字。

### 7.3 id 与 label 引用

`edges.from` / `edges.to` 先按外部 `id` 查 `id_map`，查不到再按 `label` 匹配。
两种写法可以混用：

```json
{ "edges": [
  { "from": "a1",            "to": "堡垒机" },
  { "from": "堡垒机",         "to": "a3" }
] }
```

### 7.4 链路去重

- `source == target` 跳过
- 已存在的 `source→target` 跳过（同一对节点只保留一条链路）

### 7.5 布局

新节点自动分层：入边越多的节点越靠右，同层自上而下排开。
已有节点的位置**不会被改动**，新节点从「现有最右节点 + 320px」开始排。
需要人工调整时在界面上直接拖。

### 7.6 审计

写入产生 `node.create`、`edge.create` 记录，操作人显示为凭据对应的账号。
界面「审计记录」里能看到「谁在什么时候加了哪些节点」。

---

## 8. 错误处理

### 8.1 三层错误

| 层 | 表现 | 处理 |
| --- | --- | --- |
| HTTP 层 | 401 / 403，响应体是 `{code,msg,data}` | 重新登录或换凭据，不要重试同一请求 |
| JSON-RPC 层 | `error.code` | 方法名或参数结构错了，修正请求 |
| 工具层 | `result.isError = true`，`text` 是中文消息 | 按消息调整参数或权限 |

### 8.2 JSON-RPC 错误码

| code | message | 原因 |
| --- | --- | --- |
| -32600 | 无效请求 | 缺 `method` |
| -32601 | 未实现的方法：xxx | 方法名拼错 |
| -32602 | 缺少工具名称 | `params.name` 为空 |

### 8.3 工具错误消息

| text | 原因 | 处理 |
| --- | --- | --- |
| `project_id 不能为空` | `add_nodes`/`add_edges` 没带项目 | 补上 `project_id` |
| `project_id 与 node_id 不能为空` | `update_node` 缺参 | 两者都传 |
| `项目不存在或无权访问` | 项目不存在，或当前账号无权限 | 用 `list_projects` 确认 id；换管理员凭据 |
| `nodes 与 edges 不能同时为空` | `ingest_attack_path` 两个数组都空 | 至少传一个 |
| `单次导入规模过大` | 节点 > 500 或链路 > 1000 | 拆成多次调用 |
| `nodes 格式错误：…` | 字段类型不对 | 按 §4.4 的表校验 |
| `节点不存在` | `update_node` 的 target 找不到 | 先 `search_graph` |
| `没有该项目的操作权限` | 对只读项目执行写操作 | 请管理员提权到 editor |
| `工具执行失败` | 服务端内部错误 | 看后端日志，稍后重试 |

---

## 9. 给 Agent 的提示词片段

把下面这段放进 system prompt，可显著减少无效调用：

```text
你可以通过 MCP 工具把渗透测试结果写入 RedTeam Atlas 的攻击路径拓扑。

写入规则：
1. 先调用 list_projects 判断项目是否已存在；存在就复用 project_id，不存在才用 project_name。
2. 一次能画完就用 ingest_attack_path，不要逐个 add_nodes。
3. nodes 的 label 是去重键，必须唯一且可读（用主机名或 IP，不要用「节点1」）。
4. 先用节点的 id 字段（如 a1/a2）建立引用，再在 edges 的 from/to 里用同一套 id。
5. kind 从 list_asset_types 的返回值里选；拿不准就用 host。
6. 已控制的机器 status 填 control，已发现可利用但未拿下填 attack，只有情报填 recon，受阻填 blocked。
7. edges.notes 写利用条件与凭据，status 按实际结果填。
8. 写入成功后，把返回的 editor_path 拼上域名回给用户。

不要做的事：
- 不要给同一个项目重复创建同名节点（会命中已有节点而不是新建）。
- 不要对 permission 为 viewer 的项目写入。
- 不要在 label 里放密码、口令等敏感串。
```

---

## 10. 安全与限制

- 凭据等价于网页登录，权限完全一致；为 Agent 单独建一个组员账号并按项目授权，比直接用管理员更稳妥。
- Basic 的凭据以 base64 明文出现在请求头，**只能走 HTTPS**；能存令牌时优先 Bearer。
- 所有写入都产生审计记录，操作人是凭据对应的账号，事后可追溯。
- 单次调用有规模上限（§6.4），大图请分批。
- 服务端不访问外网，不执行任何脚本内容；`notes` 等字段按纯文本存储与展示。

---

## 11. 版本与兼容

这一节说的是**协议版本**，不是软件版本。两者容易混，先分开。

### 11.1 两个「版本」不是一回事

`initialize` 的响应里同时出现两个版本字段：

```json
{
  "protocolVersion": "2025-06-18",
  "serverInfo": { "name": "redteam-atlas", "version": "4.4.1" }
}
```

| 字段 | 含义 | 谁关心 |
| --- | --- | --- |
| `protocolVersion` | MCP 协议本身的版本，用规范发布日命名 | 握手时协商，决定双方按哪套报文规则通信 |
| `serverInfo.version` | RedTeam Atlas 这个产品的版本，与 `Cargo.toml` 一致 | 排障、升级核对、界面「检查更新」 |

协议版本像「说哪一版行话」，软件版本像「这套软件的第几版」。两者独立：同一个
`2025-06-18` 下面，跑 3.1.0 和 3.2.0 都正常。

### 11.2 为什么是日期

MCP 规范的版本号就是发布日，格式 `YYYY-MM-DD`，没有语义化版本的 major.minor。
`2025-06-18` 意思是「2025 年 6 月 18 日发布的那版规范」。

目前三个正式版本，本服务端都支持：

| 协议版本 | 这一版的关键变化 |
| --- | --- |
| `2024-11-05` | 初版，传输层用 HTTP+SSE 双通道 |
| `2025-03-26` | 引入 Streamable HTTP（单端点 POST），取代双通道 |
| `2025-06-18` | 新增结构化输出与 elicitation，移除 JSON-RPC 批量；推荐 |

### 11.3 协商怎么发生

MCP 只有一次协商机会：`initialize`。

**第一步，客户端自报**：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-06-18",
    "capabilities": {},
    "clientInfo": { "name": "my-agent", "version": "1.0.0" }
  }
}
```

`params.protocolVersion` 的含义是「我会说这一版行话」。

**第二步，服务端回话**：命中白名单就原样返回（「我也说这一版」），否则返回自己最新的
（「我只会这一版，你看能不能将就」）。

**第三步，客户端决定**：按规范，客户端收到自己不支持的版本时应当断开并提示用户，
而不是硬着头皮继续。所以服务端「回落」不是万能的，它是把选择权交回给客户端。

### 11.4 本服务端的实际反应（实测）

| 客户端发送 | 服务端返回 | 结果 |
| --- | --- | --- |
| `2024-11-05` | `2024-11-05` | 原样接受 |
| `2025-03-26` | `2025-03-26` | 原样接受 |
| `2025-06-18` | `2025-06-18` | 原样接受 |
| `2026-09-01`（未来版本） | `2025-06-18` | 回落到最新支持的版本 |
| `abc`（乱填） | `2025-06-18` | 回落到最新支持的版本 |
| 不带该字段 | `2024-11-05` | 按最早版本处理 |

规则两条：白名单命中则原样回显，否则回落最新；字段缺失时取数组第一项。

### 11.5 为什么回落旧版本也不会坏

本服务端只用到三个方法，它们的报文格式在三个版本里没有差别：

| 用到的方法 | 2024-11-05 | 2025-03-26 | 2025-06-18 |
| --- | --- | --- | --- |
| `initialize` | 同 | 同 | 同 |
| `tools/list` | 同 | 同 | 同 |
| `tools/call` | 同 | 同 | 同（可加 `structuredContent`，本服务端不返回） |

各版本的实际差异都落在本服务端没碰的地方：

| 变化 | 引入版本 | 本服务端的情况 |
| --- | --- | --- |
| HTTP+SSE 双通道传输 | 2024-11-05 | 不用。`GET /mcp` 直接返回 405 |
| Streamable HTTP 单端点 | 2025-03-26 | 就按这个形态实现：一次 POST 一次响应 |
| 结构化输出 `structuredContent` | 2025-06-18 | 不返回，只用 `content[].text`，三个版本都合法 |
| elicitation、resource links | 2025-06-18 | 不涉及 |
| JSON-RPC 批量（请求体是数组） | 2025-03-26 起，2025-06-18 移除 | 不支持，发数组会得到 `-32600 无效请求` |

所以协议版本只影响「握手时的口头约定」，不影响实际能力。

### 11.6 `capabilities.tools.listChanged` 为什么恒为 false

`initialize` 响应里的能力声明：

```json
"capabilities": { "tools": { "listChanged": false } }
```

`listChanged` 回答的是：**工具列表会不会在会话进行中变化？**

| 值 | 含义 | 客户端要做什么 |
| --- | --- | --- |
| `true` | 工具会增删，变化时服务端主动发 `notifications/tools/list_changed` | 必须监听该通知，收到后重新拉 `tools/list` |
| `false` | 工具集固定不变 | 拉一次 `tools/list` 就能一直缓存，不必监听 |

本服务端的 20 个工具是编译进二进制的常量，运行期不增不减，因此恒为 `false`。
客户端据此可以省掉一次订阅和若干次重复拉取。

### 11.7 只实现 tools，其他能力没有

`-32601` 表示方法未实现。本服务端只声明并实现 tools：

| 请求 | 响应 |
| --- | --- |
| `resources/list` | `{"error":{"code":-32601,"message":"未实现的方法：resources/list"},"id":1,"jsonrpc":"2.0"}` |
| `prompts/list` | 同上 |

只依赖 resources 或 prompts 的客户端会在这一步失败，与本服务端的能力范围有关，不是配置问题。

### 11.8 握手对不上怎么排查

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| 客户端报 unsupported protocol version | 它发了未来版本，服务端回了 `2025-06-18` | 升级服务端，或在客户端配置里锁定受支持的版本 |
| 客户端连不上，日志里 `GET /mcp` 返回 405 | 客户端只支持 2024-11-05 的 SSE 传输 | 换支持 Streamable HTTP 的客户端版本 |
| 握手成功但工具调不通 | 不是版本问题 | 按第 8 章的三层错误排查 |
| 工具列表与文档不一致 | 客户端缓存了旧的 `tools/list` | 重开会话；`listChanged` 为 false，服务端不会主动推送变化 |
| 发一次批量数组就报 -32600 | 不支持 JSON-RPC 批量 | 拆成多次单条请求 |

### 11.9 想加新协议版本时改哪里

只需改一处常量：

```rust
// backend/src/mcp/mod.rs
const PROTOCOL_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];
```

数组**最后一项**是回落版本，新增版本时追加到末尾即可（记得同步改数组长度）。
改完重新构建生效，前端与数据库都不需要动。
