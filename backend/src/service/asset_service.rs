//! 组件类型与图标上传业务。

use std::path::PathBuf;

use sqlx::SqlitePool;

use crate::auth::AuthUser;
use crate::config::Config;
use crate::db::{asset_db, crypto, new_id};
use crate::error::AppError;
use crate::model::misc::{AssetTypeRequest, UploadRequest, UploadResult};
use crate::model::AssetType;
use crate::service::{now_ms, tidy, valid_color};

const MAX_CUSTOM_TYPES_PER_USER: i64 = 60;
const MAX_LABEL_CHARS: usize = 24;

/// 允许的图片类型 → 扩展名
fn ext_of(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

/// 内置组件清单（首次启动写入库，后续自定义组件与之合并）
pub fn builtin_types() -> Vec<&'static str> {
    vec![
        "attacker", "entry", "web", "cloud", "host", "domain", "database", "workstation", "network",
        "firewall", "pivot", "credential", "vuln", "flag",
    ]
}

/// 内置组件的中文名与配色
fn builtin_meta(slug: &str) -> (&'static str, &'static str) {
    match slug {
        "attacker" => ("攻击者", "#c96f7a"),
        "entry" => ("攻击入口", "#d9a441"),
        "web" => ("Web 应用", "#7fb0d8"),
        "cloud" => ("云资源", "#8fb8c9"),
        "host" => ("主机服务器", "#9fb3c8"),
        "domain" => ("域控 / AD", "#b49bd8"),
        "database" => ("数据库", "#87c6a8"),
        "workstation" => ("工作站", "#9aa8b8"),
        "network" => ("网络 / 网段", "#8fa3b8"),
        "firewall" => ("防火墙 / WAF", "#c98b7f"),
        "pivot" => ("跳板 / 隧道", "#c9a87f"),
        "credential" => ("凭证 / 账号", "#d8b46a"),
        "vuln" => ("漏洞利用点", "#d97f7f"),
        "flag" => ("战果 / 目标", "#d9a441"),
        _ => ("自定义组件", "#8892a0"),
    }
}

/// 启动时补齐内置组件
pub async fn seed(pool: &SqlitePool) -> Result<(), AppError> {
    // 旧版本内置过 note（备注 / 分组），这里清掉，避免组件库里留一个用不上的类型
    let _ = sqlx::query("DELETE FROM asset_types WHERE slug = 'note' AND builtin = 1")
        .execute(pool)
        .await;

    let at = now_ms();
    let items: Vec<AssetType> = builtin_types()
        .into_iter()
        .map(|slug| {
            let (label, color) = builtin_meta(slug);
            AssetType {
                slug: slug.to_string(),
                label: label.to_string(),
                color: color.to_string(),
                icon_url: None,
                builtin: true,
                created_by: None,
                created_at: at,
                updated_at: at,
            }
        })
        .collect();
    asset_db::seed_builtin(pool, &items).await
}

/// 组件清单
pub async fn list(pool: &SqlitePool) -> Result<Vec<AssetType>, AppError> {
    asset_db::list_types(pool).await
}

/// 新建自定义组件
pub async fn create(
    pool: &SqlitePool,
    actor: &AuthUser,
    req: AssetTypeRequest,
) -> Result<AssetType, AppError> {
    let label = tidy(&req.label, MAX_LABEL_CHARS);
    if label.is_empty() {
        return Err(AppError::BadRequest("组件名称不能为空".to_string()));
    }
    if asset_db::count_custom_types(pool, &actor.id).await? >= MAX_CUSTOM_TYPES_PER_USER {
        return Err(AppError::BadRequest("自定义组件数量已达上限".to_string()));
    }

    let color = if valid_color(&req.color) {
        req.color.trim().to_string()
    } else {
        "#d9a441".to_string()
    };
    let icon_url = normalize_icon(req.icon_url.as_deref());

    // 旧版本内置过 note（备注 / 分组），这里清掉，避免组件库里留一个用不上的类型
    let _ = sqlx::query("DELETE FROM asset_types WHERE slug = 'note' AND builtin = 1")
        .execute(pool)
        .await;

    let at = now_ms();
    let item = AssetType {
        slug: format!("ct-{}", &new_id("")[..8]),
        label,
        color,
        icon_url,
        builtin: false,
        created_by: Some(actor.id.clone()),
        created_at: at,
        updated_at: at,
    };
    asset_db::insert_type(pool, &item).await?;
    Ok(item)
}

/// 修改自定义组件（创建者或管理员）
pub async fn update(
    pool: &SqlitePool,
    actor: &AuthUser,
    slug: &str,
    req: AssetTypeRequest,
) -> Result<AssetType, AppError> {
    let existing = asset_db::find_type(pool, slug)
        .await?
        .ok_or_else(|| AppError::NotFound("组件不存在".to_string()))?;

    if existing.builtin {
        return Err(AppError::Forbidden("内置组件不可修改".to_string()));
    }
    let is_owner = existing.created_by.as_deref() == Some(actor.id.as_str());
    if !is_owner && !actor.is_admin() {
        return Err(AppError::Forbidden("只能修改自己创建的组件".to_string()));
    }

    let label = tidy(&req.label, MAX_LABEL_CHARS);
    if label.is_empty() {
        return Err(AppError::BadRequest("组件名称不能为空".to_string()));
    }
    let color = if valid_color(&req.color) {
        req.color.trim().to_string()
    } else {
        existing.color.clone()
    };
    let icon = normalize_icon(req.icon_url.as_deref()).or(existing.icon_url.clone());

    asset_db::update_type(pool, slug, &label, &color, icon.as_deref(), now_ms()).await?;

    asset_db::find_type(pool, slug)
        .await?
        .ok_or_else(|| AppError::NotFound("组件不存在".to_string()))
}

/// 删除自定义组件
pub async fn delete(pool: &SqlitePool, actor: &AuthUser, slug: &str) -> Result<(), AppError> {
    let existing = asset_db::find_type(pool, slug)
        .await?
        .ok_or_else(|| AppError::NotFound("组件不存在".to_string()))?;
    if existing.builtin {
        return Err(AppError::Forbidden("内置组件不可删除".to_string()));
    }
    let is_owner = existing.created_by.as_deref() == Some(actor.id.as_str());
    if !is_owner && !actor.is_admin() {
        return Err(AppError::Forbidden("只能删除自己创建的组件".to_string()));
    }
    asset_db::delete_type(pool, slug).await
}

/// 内置图标 key（与前端内置图标集一一对应）
const BUILTIN_ICONS: [&str; 5] = ["target", "bug", "server", "cloud", "key"];

/// 图标地址只接受本站上传路径或内置图标 key，避免服务端被诱导访问任意外链
fn normalize_icon(url: Option<&str>) -> Option<String> {
    let value = url?.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(key) = value.strip_prefix("builtin:") {
        return BUILTIN_ICONS
            .contains(&key)
            .then(|| format!("builtin:{key}"));
    }
    if let Some(id) = value.strip_prefix("/api/uploads/") {
        if valid_upload_id(id) {
            return Some(format!("/api/uploads/{id}"));
        }
    }
    None
}

/// 上传 ID 只允许 `img_` + 32 位十六进制，杜绝路径穿越
pub fn valid_upload_id(id: &str) -> bool {
    id.len() == 36 && id.starts_with("img_") && id[4..].chars().all(|c| c.is_ascii_hexdigit())
}

/// 保存上传的图标
pub async fn save_upload(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    req: UploadRequest,
) -> Result<UploadResult, AppError> {
    let ext = ext_of(&req.mime)
        .ok_or_else(|| AppError::BadRequest("只支持 PNG / JPEG / WebP / GIF / SVG 图片".to_string()))?;

    let bytes = crypto::unb64(&req.data_base64)?;
    if bytes.is_empty() {
        return Err(AppError::BadRequest("文件内容为空".to_string()));
    }
    if bytes.len() > cfg.upload_max_bytes {
        return Err(AppError::BadRequest(format!(
            "文件超过 {} KB 限制",
            cfg.upload_max_bytes / 1024
        )));
    }

    // SVG 里的脚本与事件属性在浏览器直接打开时可能执行，这里直接拒绝
    if ext == "svg" {
        let text = String::from_utf8_lossy(&bytes).to_lowercase();
        let dangerous = ["<script", "javascript:", "onload=", "onerror=", "<iframe", "<foreignobject"];
        if dangerous.iter().any(|needle| text.contains(needle)) {
            return Err(AppError::BadRequest("SVG 含有可执行内容，已拒绝".to_string()));
        }
    }

    let id = format!("img_{}", &new_id("")[..32]);
    let storage = format!("{id}.{ext}");
    let path: PathBuf = cfg.upload_dir.join(&storage);
    std::fs::write(&path, &bytes).map_err(|e| AppError::Internal(format!("写入文件失败: {e}")))?;

    let filename = tidy(&req.filename, 120);
    asset_db::insert_upload(
        pool,
        &id,
        if filename.is_empty() { &storage } else { &filename },
        &req.mime,
        bytes.len() as i64,
        &storage,
        &actor.id,
        now_ms(),
    )
    .await?;

    Ok(UploadResult {
        url: format!("/api/pub/uploads/{id}?sig={}", sign_upload(&cfg.master_key, &id)),
        id,
        bytes: bytes.len(),
        mime: req.mime,
    })
}

/// 上传内容的短签名：HMAC(master_key, "atlas-upload-v1" || id) 取前 16 字节。
///
/// 正文里只存带签名的短 URL，浏览器可以缓存图片，报告导出时再按需取回。
/// 签名不随登录状态变化，重装前端或换账号都不会让历史报告里的图失效。
pub fn sign_upload(master_key: &[u8; 32], id: &str) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(master_key).expect("HMAC 接受任意长度密钥");
    mac.update(b"atlas-upload-v1");
    mac.update(id.as_bytes());
    let digest = mac.finalize().into_bytes();
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// 校验上传签名，比较用常量时间
pub fn verify_upload(master_key: &[u8; 32], id: &str, sig: &str) -> bool {
    let expected = sign_upload(master_key, id);
    if expected.len() != sig.len() {
        return false;
    }
    expected
        .bytes()
        .zip(sig.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// 读取图标内容
pub async fn read_upload(
    pool: &SqlitePool,
    cfg: &Config,
    id: &str,
) -> Result<(String, Vec<u8>), AppError> {
    if !valid_upload_id(id) {
        return Err(AppError::NotFound("文件不存在".to_string()));
    }
    let (mime, storage, _filename) = asset_db::find_upload(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("文件不存在".to_string()))?;

    // storage 由服务端生成，仍再做一次白名单校验
    if storage.contains('/') || storage.contains('\\') || storage.contains("..") {
        return Err(AppError::NotFound("文件不存在".to_string()));
    }
    let path = cfg.upload_dir.join(&storage);
    let bytes = std::fs::read(&path).map_err(|_| AppError::NotFound("文件不存在".to_string()))?;
    Ok((mime, bytes))
}