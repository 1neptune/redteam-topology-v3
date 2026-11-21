//! 统一错误类型：所有业务错误都映射为 { code, msg, data } 响应。
//! 禁止把内部错误细节（SQL、路径等）回传给客户端。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
    #[error("服务器内部错误")]
    Internal(String),
}

impl AppError {
    /// 业务错误码与 HTTP 状态码保持一致，便于前端统一拦截。
    pub fn code(&self) -> u16 {
        match self {
            AppError::BadRequest(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::NotFound(_) => 404,
            AppError::Conflict(_) => 409,
            AppError::Database(_) | AppError::Internal(_) => 500,
        }
    }

    /// 对外文案：数据库/内部错误统一模糊化，避免信息泄露。
    pub fn message(&self) -> String {
        match self {
            AppError::BadRequest(m)
            | AppError::Unauthorized(m)
            | AppError::Forbidden(m)
            | AppError::NotFound(m)
            | AppError::Conflict(m) => m.clone(),
            AppError::Database(e) => {
                tracing::error!(error = %e, "数据库操作失败");
                "服务器内部错误".to_string()
            }
            AppError::Internal(e) => {
                tracing::error!(error = %e, "内部错误");
                "服务器内部错误".to_string()
            }
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    code: u16,
    msg: String,
    data: Option<()>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let code = self.code();
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = ErrorBody {
            code,
            msg: self.message(),
            data: None,
        };
        (status, axum::Json(body)).into_response()
    }
}