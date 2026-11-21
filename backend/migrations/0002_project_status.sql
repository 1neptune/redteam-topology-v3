-- 项目状态：active（进行中）/ done（已完成）
ALTER TABLE projects ADD COLUMN status TEXT NOT NULL DEFAULT 'active';