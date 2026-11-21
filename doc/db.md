# 数据库设计（SQLite · sqlx migrate）

## 1. ER 关系

```
users 1 ──< projects (owner_id)
users 1 ──< project_members >── 1 projects
projects 1 ──< audit_log
projects 1 ──< presence
users 1 ──< asset_types (created_by)
users 1 ──< uploads (created_by)
```

| 关系 | 基数 | 说明 |
| --- | --- | --- |
| users → projects | 1:N | 项目创建者（owner_id），删除用户时项目保留 |
| projects ↔ users | M:N | `project_members` 承载 owner/editor/viewer 三级权限 |
| projects → audit_log | 1:N | 项目内所有变更轨迹，项目删除时级联清理 |
| projects → presence | 1:N | 在线成员与光标，随项目删除 |
| users → asset_types | 1:N | 自定义组件创建者 |
| users → uploads | 1:N | 图标上传者 |

## 2. 关键设计说明

- **时间**：全部为毫秒时间戳（`INTEGER`），避免时区歧义。
- **布尔**：`disabled`、`builtin` 用 `0/1`。
- **主键**：`u_` / `p_` / `a_` / `img_` 前缀 + 16 位随机十六进制，便于日志辨识。
- **加密**：`projects` 的拓扑不存明文，`data_cipher` / `data_iv` / `data_tag` 三列由应用层
  AES-256-GCM 生成；密钥由环境变量 `ATLAS_SECRET` 经 argon2 派生 32 字节，未设置时读取数据目录的 `secret.key`。
- **JWT 失效**：`users.token_version` 写入 JWT 声明，改密/停用时自增，校验时比对，实现
  「改密即下线」而无需服务端会话表。
- **审计不脱敏**：`detail` 保存字段级 diff（JSON 字符串），超过 12KB 截断。
- **搜索**：`projects.name` / `description` 使用 `LIKE`（参数绑定，无注入面）。

## 3. 建表 SQL（sqlx migrate 原文）

文件：`backend/migrations/0001_init.sql`（建表）、`backend/migrations/0002_project_status.sql`（补 `projects.status`）

```sql
CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    password_hash TEXT NOT NULL,
    token_version INTEGER NOT NULL DEFAULT 1,
    disabled      INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL,
    last_login_at INTEGER
);

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    owner_id    TEXT NOT NULL REFERENCES users (id),
    version     INTEGER NOT NULL DEFAULT 1,
    node_count  INTEGER NOT NULL DEFAULT 0,
    edge_count  INTEGER NOT NULL DEFAULT 0,
    data_cipher BLOB NOT NULL,
    data_iv     BLOB NOT NULL,
    data_tag    BLOB NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    updated_by  TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'active'
);

CREATE TABLE project_members (
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    permission TEXT NOT NULL CHECK (permission IN ('owner', 'editor', 'viewer')),
    granted_by TEXT,
    granted_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, user_id)
);

CREATE TABLE audit_log (
    id           TEXT PRIMARY KEY,
    project_id   TEXT REFERENCES projects (id) ON DELETE CASCADE,
    user_id      TEXT,
    user_name    TEXT NOT NULL,
    action       TEXT NOT NULL,
    target_type  TEXT NOT NULL,
    target_id    TEXT NOT NULL DEFAULT '',
    target_label TEXT NOT NULL DEFAULT '',
    summary      TEXT NOT NULL,
    detail       TEXT,
    created_at   INTEGER NOT NULL
);

CREATE TABLE asset_types (
    slug       TEXT PRIMARY KEY,
    label      TEXT NOT NULL,
    color      TEXT NOT NULL,
    icon_url   TEXT,
    builtin    INTEGER NOT NULL DEFAULT 0,
    created_by TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE uploads (
    id         TEXT PRIMARY KEY,
    filename   TEXT NOT NULL,
    mime       TEXT NOT NULL,
    bytes      INTEGER NOT NULL,
    storage    TEXT NOT NULL,
    created_by TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE presence (
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL,
    user_name  TEXT NOT NULL,
    cursor_x   REAL NOT NULL DEFAULT 0,
    cursor_y   REAL NOT NULL DEFAULT 0,
    seen_at    INTEGER NOT NULL,
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX idx_projects_updated ON projects (updated_at DESC);
CREATE INDEX idx_projects_owner ON projects (owner_id);
CREATE INDEX idx_members_user ON project_members (user_id);
CREATE INDEX idx_audit_project ON audit_log (project_id, created_at DESC);
CREATE INDEX idx_audit_created ON audit_log (created_at DESC);
CREATE INDEX idx_presence_seen ON presence (project_id, seen_at DESC);
```

## 4. 迁移执行

应用启动时通过 `sqlx::migrate!("./migrations")` 自动执行，具备幂等性（sqlx 记录 `_sqlx_migrations` 表）。

已应用的迁移文件不能再改——sqlx 会比对文件哈希，改动后启动直接报 `VersionMismatch` 拒绝运行。要调整表结构就新增一个迁移。

| 迁移 | 内容 |
| --- | --- |
| `0001_init.sql` | 建全部基础表与索引 |
| `0002_project_status.sql` | `projects` 增加 `status`（`active` / `done`），默认进行中 |
也可用 CLI 手动执行：

```bash
cargo install sqlx-cli --no-default-features --features sqlite
sqlx database create
sqlx migrate run
```

## 5. 备份与恢复

- SQLite 使用 WAL 模式，备份需同时包含 `atlas.db`、`atlas.db-wal`、`atlas.db-shm`，
  或执行 `sqlite3 atlas.db ".backup backup.db"`。
- 加密后的拓扑若丢失 `ATLAS_SECRET` 将无法解密，密钥必须与数据库一同备份。