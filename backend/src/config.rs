//! 运行时配置：全部来自环境变量，禁止硬编码密钥与路径。

use std::path::PathBuf;
use std::sync::Arc;

use argon2::Argon2;

/// 应用配置（启动时读取一次，之后只读共享）
pub struct Config {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_ttl_secs: i64,
    pub upload_dir: PathBuf,
    pub upload_max_bytes: usize,
    pub bootstrap_admin: String,
    pub bootstrap_password: Option<String>,
    /// 更新清单文件（升级脚本写入，离线环境兜底）
    pub update_file: PathBuf,
    /// 官网仓库 owner/name，用于「检查更新」查询 GitHub Releases
    pub update_repo: String,
    /// 拓扑加密主密钥（AES-256）
    pub master_key: [u8; 32],
}

/// 读取环境变量并提供默认值
fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn from_env() -> Result<Arc<Self>, String> {
        let data_dir = PathBuf::from(env_or("ATLAS_DATA_DIR", "./data"));
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
        let upload_dir = data_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).map_err(|e| format!("创建上传目录失败: {e}"))?;
        let update_file = std::env::var("ATLAS_UPDATE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("update.json"));

        let database_url = env_or(
            "ATLAS_DATABASE_URL",
            &format!("sqlite://{}/atlas.db?mode=rwc", data_dir.display()),
        );

        // JWT 密钥：生产环境必须显式配置；未配置时生成随机值（重启后旧令牌失效）
        let jwt_secret = match std::env::var("ATLAS_JWT_SECRET") {
            Ok(value) if value.len() >= 16 => value,
            _ => {
                tracing::warn!("未设置 ATLAS_JWT_SECRET，使用本次启动的随机密钥（重启后需重新登录）");
                uuid::Uuid::new_v4().to_string()
            }
        };

        // 主密钥：由 ATLAS_SECRET 派生；未设置时读取/生成 data/secret.key
        let secret = std::env::var("ATLAS_SECRET").unwrap_or_default();
        let key_file = data_dir.join("secret.key");
        let master_key = if secret.trim().is_empty() {
            if key_file.exists() {
                let bytes = std::fs::read(&key_file).map_err(|e| format!("读取密钥文件失败: {e}"))?;
                if bytes.len() != 32 {
                    return Err("secret.key 长度必须为 32 字节".to_string());
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                key
            } else {
                use aes_gcm::aead::rand_core::RngCore;
                let mut key = [0u8; 32];
                let mut rng = aes_gcm::aead::OsRng;
                rng.fill_bytes(&mut key);
                std::fs::write(&key_file, key).map_err(|e| format!("写入密钥文件失败: {e}"))?;
                tracing::warn!("已生成新的加密主密钥：{}（请与数据库一同备份）", key_file.display());
                key
            }
        } else {
            // 口令派生：Argon2id，固定盐保证多实例一致
            let mut key = [0u8; 32];
            Argon2::default()
                .hash_password_into(secret.as_bytes(), b"redteam-atlas-kdf-v3", &mut key)
                .map_err(|e| format!("主密钥派生失败: {e}"))?;
            key
        };

        Ok(Arc::new(Self {
            port: env_or("PORT", "8080").parse::<u16>().unwrap_or(8080),
            host: env_or("HOST", "0.0.0.0"),
            database_url,
            jwt_secret,
            jwt_ttl_secs: env_or("ATLAS_JWT_TTL_SECS", "604800").parse::<i64>().unwrap_or(604800),
            upload_dir,
            upload_max_bytes: env_or("ATLAS_UPLOAD_MAX_BYTES", "2097152")
                .parse::<usize>()
                .unwrap_or(2 * 1024 * 1024),
            bootstrap_admin: env_or("ATLAS_ADMIN_USER", "admin"),
            bootstrap_password: std::env::var("ATLAS_ADMIN_PASSWORD").ok().filter(|v| !v.is_empty()),
            update_file,
            update_repo: env_or("ATLAS_UPDATE_REPO", "1neptune/redteam-topology-v3"),
            master_key,
        }))
    }
}