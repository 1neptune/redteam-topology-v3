//! RedTeam Atlas 后端入口：装配配置、数据库、路由并启动 HTTP 服务。

mod auth;
mod config;
mod db;
mod error;
mod handler;
mod mcp;
mod middleware;
mod model;
mod response;
mod router;
mod service;
mod state;

use sqlx::sqlite::SqlitePoolOptions;

use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 读取 .env（存在时），便于本地开发与 Docker 外的常驻部署
    if let Err(err) = dotenvy::dotenv() {
        eprintln!("[env] 未加载 .env（{err}），使用默认配置");
    }

    let cfg = config::Config::from_env().map_err(|e| format!("配置错误：{e}"))?;

    // 控制台一份、error.log 一份：ERROR 及以上同时落盘，方便事后排查
    service::run_log::init_error_writer(&cfg);
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        use tracing_subscriber::Layer;

        let console = tracing_subscriber::fmt::layer().with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=warn".into()),
        );
        let file = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_writer(service::run_log::ErrorWriter)
            .with_filter(tracing_subscriber::filter::LevelFilter::ERROR);
        tracing_subscriber::registry()
            .with(console)
            .with(file)
            .init();
    }

    // SQLite 连接池：WAL + 外键约束 + 忙等超时
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect(&cfg.database_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;
    sqlx::query("PRAGMA busy_timeout = 5000").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous = NORMAL").execute(&pool).await?;

    // 迁移脚本随二进制编译进来，启动即执行
    sqlx::migrate!("./migrations").run(&pool).await?;

    service::asset_service::seed(&pool).await?;

    let bootstrap = service::auth_service::ensure_admin(&pool, &cfg).await?;
    let users = db::user_db::count(&pool).await.unwrap_or(0);
    let projects: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM projects")
        .fetch_one(&pool)
        .await
        .map(|(c,)| c)
        .unwrap_or(0);

    let app = router::build(AppState::new(pool.clone(), cfg.clone()));

    // 每 5 分钟清理过期的心跳记录，避免 presence 表无限增长
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            ticker.tick().await;
            let before = service::now_ms() - 60_000;
            if let Err(err) = db::project_db::presence_cleanup(&cleanup_pool, before).await {
                tracing::warn!(error = %err, "清理在线状态失败");
            }
        }
    });

    let addr = format!("{}:{}", cfg.host, cfg.port);
    eprintln!("[boot] 监听 {addr}，数据目录 {}", cfg.upload_dir.display());
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // 启动摘要：版本、可访问地址、管理员账号，其余一律进 tracing
    // 一律走 stderr：容器里 stdout 可能被缓冲，日志丢了就查不出问题
    eprintln!();
    eprintln!("RedTeam Atlas {}", env!("CARGO_PKG_VERSION"));
    eprintln!("{}", serve_url(&cfg.host, cfg.port));
    eprintln!();
    match bootstrap {
        Some((user, password)) => {
            // 首次建库：此刻的口令就是刚生成的初始口令
            eprintln!("初始管理员");
            eprintln!("  用户名: {user}");
            eprintln!("  口令  : {password}");
        }
        None => {
            // 账号已存在：口令是哈希。若 .env 里的 ATLAS_ADMIN_PASSWORD 正好能验证通过，
            // 说明配置里的就是当前口令，直接如实显示；否则只给名称与重置方式。
            let admin = current_admin_name(&pool).await;
            let from_env = match cfg.bootstrap_password.as_deref() {
                Some(value) if !value.is_empty() => {
                    service::auth_service::verify_credentials(&pool, &admin, value)
                        .await
                        .is_ok()
                        .then(|| value.to_string())
                }
                _ => None,
            };
            eprintln!("管理员");
            eprintln!("  用户名: {admin}");
            eprintln!(
                "  口令  : {}",
                from_env.unwrap_or_else(|| "已设置，忘记可在「用户管理」里重置".to_string())
            );
        }
    }
    eprintln!();
    tracing::debug!(
        database = %cfg.database_url,
        users,
        projects,
        update_file = %cfg.update_file.display(),
        "服务已就绪：账号由管理员创建并授权；MCP 端点为 POST /mcp"
    );

    // 带上连接信息，中间件才能取到来源 IP
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// 库里已有的管理员用户名（取最早创建的那个）
async fn current_admin_name(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_as::<_, (String,)>(
        "SELECT username FROM users WHERE role = 'admin' ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|(name,)| name)
    .unwrap_or_else(|| "admin".to_string())
}

/// 打印出来能直接点开的地址：监听 0.0.0.0 时用本机内网 IP，否则用监听地址
fn serve_url(host: &str, port: u16) -> String {
    let shown = match host {
        "0.0.0.0" | "::" | "" => local_ip().unwrap_or_else(|| "localhost".to_string()),
        other => other.to_string(),
    };
    format!("http://{shown}:{port}")
}

/// 取本机对外内网地址：连一个外部地址但不发数据，读 socket 的本地端即可
fn local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("223.5.5.5:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

/// 优雅退出：Ctrl+C 与容器的 SIGTERM 都要能关掉监听。
///
/// 只监听 SIGINT 的话，`docker stop` 发的 SIGTERM 会先超时 10 秒再被 SIGKILL，
/// 数据与在线状态都来不及收尾。
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "监听 Ctrl+C 失败");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "监听 SIGTERM 失败");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("收到 Ctrl+C，开始关闭"),
        _ = terminate => tracing::info!("收到 SIGTERM，开始关闭"),
    }
}