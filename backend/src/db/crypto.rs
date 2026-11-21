//! 口令哈希与拓扑加密的原语。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use rand::RngCore;

use crate::error::AppError;

/// 口令哈希（Argon2id）
pub fn hash_password(password: &str) -> Result<String, AppError> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    let salt = SaltString::generate(&mut OsRng);
    argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("口令哈希失败: {e}")))
}

/// 口令校验（定长比较由 argon2 内部完成）
pub fn verify_password(hash: &str, password: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    match PasswordHash::new(hash) {
        Ok(parsed) => argon2::Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// 拓扑加密结果：密文、IV、认证标签分开存三列
pub struct Sealed {
    pub cipher: Vec<u8>,
    pub iv: Vec<u8>,
    pub tag: Vec<u8>,
}

/// AES-256-GCM 加密
pub fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<Sealed, AppError> {
    let aes = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut iv = [0u8; 12];
    rand::rng().fill_bytes(&mut iv);
    let mut payload = aes
        .encrypt(Nonce::from_slice(&iv), plaintext)
        .map_err(|e| AppError::Internal(format!("加密失败: {e}")))?;
    if payload.len() < 16 {
        return Err(AppError::Internal("密文长度异常".to_string()));
    }
    let tag = payload.split_off(payload.len() - 16);
    Ok(Sealed {
        cipher: payload,
        iv: iv.to_vec(),
        tag,
    })
}

/// AES-256-GCM 解密
pub fn open(key: &[u8; 32], cipher: &[u8], iv: &[u8], tag: &[u8]) -> Result<Vec<u8>, AppError> {
    if iv.len() != 12 || tag.len() != 16 {
        return Err(AppError::Internal("密文头损坏".to_string()));
    }
    let aes = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut payload = cipher.to_vec();
    payload.extend_from_slice(tag);
    aes.decrypt(Nonce::from_slice(iv), payload.as_ref())
        .map_err(|_| AppError::Internal("拓扑解密失败，请检查 ATLAS_SECRET".to_string()))
}

/// base64 解码
pub fn unb64(data: &str) -> Result<Vec<u8>, AppError> {
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::BadRequest("base64 内容格式错误".to_string()))
}