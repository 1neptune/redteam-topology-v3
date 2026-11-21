-- RedTeam Atlas v3 · 初始结构
-- 约定：时间戳统一为毫秒整数（INTEGER），布尔用 0/1，主键为带前缀的随机字符串。

CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    password_hash TEXT NOT NULL,
    -- 修改口令时自增，使旧 JWT 立即失效
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
    -- 拓扑 JSON 以 AES-256-GCM 加密后分三列存放
    data_cipher BLOB NOT NULL,
    data_iv     BLOB NOT NULL,
    data_tag    BLOB NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    updated_by  TEXT NOT NULL DEFAULT ''
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

-- 在线成员与光标（前端轮询，避免长连接带来的资源占用）
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