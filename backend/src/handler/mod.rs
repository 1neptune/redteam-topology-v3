//! Handler 层：只做入参校验、组装响应，不写 SQL、不写跨模块业务。

pub mod asset_handler;
pub mod audit_handler;
pub mod auth_handler;
pub mod project_handler;
pub mod system_handler;
pub mod user_handler;

use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use crate::error::AppError;

/// 自定义 JSON 提取器：把 axum 默认的拒绝响应替换为统一 { code, msg, data } 结构
pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(ApiJson(value)),
            Err(rejection) => Err(AppError::BadRequest(format!(
                "请求体格式错误：{}",
                rejection.body_text()
            ))),
        }
    }
}

impl<T> std::ops::Deref for ApiJson<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}