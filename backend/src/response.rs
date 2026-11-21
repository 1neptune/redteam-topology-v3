//! 统一响应包装：{ code, msg, data }。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: u16,
    pub msg: String,
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 200,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::OK);
        (status, axum::Json(self)).into_response()
    }
}

/// 无数据体的成功响应。
pub fn ok_empty() -> ApiResponse<()> {
    ApiResponse {
        code: 200,
        msg: "ok".to_string(),
        data: None,
    }
}