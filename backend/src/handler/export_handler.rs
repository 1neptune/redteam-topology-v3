//! 导出文件下载：`/api/pub/exports/{name}` 直接读文件返回。
//!
//! 导出件里只有报告内容，不含凭据，所以走公开路由，不加签名；
//! 文件名由服务端生成（随机 uuid + 扩展名），外部拿不到别人的文件名。

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::error::AppError;
use crate::state::AppState;

/// 取导出文件
pub async fn get_export(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, AppError> {
    // 只允许「uuid.扩展名」这种形状，挡住 ../ 之类的路径穿越
    let ok = !name.is_empty()
        && name.len() <= 80
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..");
    if !ok {
        return Err(AppError::NotFound("导出文件不存在".into()));
    }

    let path = state.cfg.upload_dir.join("exports").join(&name);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound("导出文件不存在或已过期".into()))?;

    let mime = match name.rsplit('.').next().unwrap_or("") {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pdf" => "application/pdf",
        "png" => "image/png",
        _ => "application/octet-stream",
    };

    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=600"),
        ],
        bytes,
    )
        .into_response())
}