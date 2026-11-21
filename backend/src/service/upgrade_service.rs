//! 一键更新：从官网 Releases 取最新版本与二进制，落盘并生成重启脚本。
//!
//! 流程分两段，中间隔着一次进程退出：
//! 1. 本进程下载新二进制到 `{数据目录}/upgrade/`，写下替换脚本，拉起脚本后自行退出；
//! 2. 脚本等本进程退出，备份旧文件、替换、再启动新进程。
//!
//! 之所以不在进程内直接替换：Windows 不允许改写正在运行的 exe，
//! 外部脚本是唯一可靠的落点。容器部署不走这条路，交给 `docker compose pull`。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;
use crate::error::AppError;

/// 一键更新的结果，直接回给界面
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeResult {
    pub from: String,
    pub to: String,
    /// 下载到的资产文件名
    pub asset: String,
    /// 落盘路径
    pub staged: String,
    /// 字节数
    pub bytes: u64,
    /// 替换脚本路径
    pub script: String,
    /// 是否已拉起脚本（拉起后本进程会退出）
    pub restarting: bool,
}

/// 目标平台的可执行文件名
fn platform_hint() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// 容器里自更新没有意义：宿主机的镜像才是要换的东西
fn in_container() -> bool {
    Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists()
}

/// 比较版本号，a 是否比 b 新
pub fn is_newer(a: &str, b: &str) -> bool {
    let parse = |raw: &str| -> Vec<u64> {
        raw.trim()
            .trim_start_matches(['v', 'V'])
            .split(['.', '-', '+'])
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let left = parse(a);
    let right = parse(b);
    for index in 0..left.len().max(right.len()) {
        let l = left.get(index).copied().unwrap_or(0);
        let r = right.get(index).copied().unwrap_or(0);
        if l != r {
            return l > r;
        }
    }
    false
}

/// 从 Releases 的资产列表里挑出本平台的可执行文件
fn pick_asset(assets: &serde_json::Value) -> Option<(String, String)> {
    let hint = platform_hint();
    assets
        .as_array()?
        .iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let url = item.get("browser_download_url")?.as_str()?.to_string();
            Some((name, url))
        })
        .find(|(name, _)| {
            let lower = name.to_lowercase();
            if lower.contains(".sha256") || lower.contains(".txt") || lower.contains(".zip") {
                return false;
            }
            lower.contains(hint)
        })
}

/// 取 GitHub Releases 的最新发布
fn latest_release(repo: &str) -> Result<serde_json::Value, AppError> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let response = ureq::get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "redteam-atlas")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|err| match err {
            ureq::Error::Status(404, _) => {
                AppError::BadRequest(format!("{repo} 仓库尚未发布正式版本"))
            }
            ureq::Error::Status(code, _) => {
                AppError::BadRequest(format!("访问官网失败（HTTP {code}）"))
            }
            other => AppError::BadRequest(format!("访问官网失败：{other}")),
        })?;
    response
        .into_json::<serde_json::Value>()
        .map_err(|err| AppError::BadRequest(format!("解析发布信息失败：{err}")))
}

/// 下载单个资产到目标路径
fn download(url: &str, target: &Path) -> Result<u64, AppError> {
    let response = ureq::get(url)
        .set("User-Agent", "redteam-atlas")
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|err| AppError::BadRequest(format!("下载失败：{err}")))?;

    let mut reader = response.into_reader();
    let mut file = fs::File::create(target)
        .map_err(|err| AppError::Internal(format!("写入升级文件失败：{err}")))?;
    let bytes = std::io::copy(&mut reader, &mut file)
        .map_err(|err| AppError::Internal(format!("下载中断：{err}")))?;
    file.flush()
        .map_err(|err| AppError::Internal(format!("落盘失败：{err}")))?;
    if bytes < 1024 {
        return Err(AppError::BadRequest("下载内容过小，可能不是可执行文件".into()));
    }
    Ok(bytes)
}

/// Windows 替换脚本：等本进程退出 → 备份 → 替换 → 启动，失败则回滚
fn windows_script(staged: &Path, exe: &Path, pid: u32) -> String {
    let staged = staged.display().to_string().replace('\'', "''");
    let exe_path = exe.display().to_string().replace('\'', "''");
    let dir = exe.parent().map(|p| p.display().to_string()).unwrap_or_default().replace('\'', "''");
    format!(
        r#"$ErrorActionPreference = 'Stop'
$staged = '{staged}'
$exe = '{exe_path}'
$dir = '{dir}'
$bak = "$exe.bak"
try {{ Wait-Process -Id {pid} -Timeout 120 -ErrorAction SilentlyContinue }} catch {{}}
Start-Sleep -Milliseconds 900
Copy-Item $exe $bak -Force -ErrorAction SilentlyContinue
try {{
    Move-Item $staged $exe -Force
    Start-Process -FilePath $exe -WorkingDirectory $dir
}} catch {{
    if (Test-Path $bak) {{ Copy-Item $bak $exe -Force }}
    Start-Process -FilePath $exe -WorkingDirectory $dir
}}
"#
    )
}

/// Linux / macOS 替换脚本
fn unix_script(staged: &Path, exe: &Path, pid: u32) -> String {
    let dir = exe.parent().map(|p| p.display().to_string()).unwrap_or_else(|| ".".into());
    format!(
        r#"#!/bin/sh
set -e
staged="{staged}"
exe="{exe}"
dir="{dir}"
while kill -0 {pid} 2>/dev/null; do sleep 0.5; done
sleep 1
cp "$exe" "$exe.bak" 2>/dev/null || true
if ! mv -f "$staged" "$exe"; then
    [ -f "$exe.bak" ] && cp -f "$exe.bak" "$exe"
    exit 1
fi
chmod +x "$exe"
cd "$dir"
nohup "$exe" >/dev/null 2>&1 &
"#,
        staged = staged.display(),
        exe = exe.display(),
        dir = dir,
        pid = pid
    )
}

/// 拉起替换脚本（脱离本进程，父进程退出后继续跑）
fn spawn_script(script: &Path) -> Result<(), AppError> {
    #[cfg(windows)]
    {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .spawn()
            .map_err(|err| AppError::Internal(format!("启动更新脚本失败：{err}")))?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("sh")
            .arg(script)
            .spawn()
            .map_err(|err| AppError::Internal(format!("启动更新脚本失败：{err}")))?;
    }
    Ok(())
}

/// 执行一键更新
pub fn upgrade(cfg: &Config) -> Result<UpgradeResult, AppError> {
    if in_container() {
        // 容器不能重建自身：把版本对比做出来，让宿主侧脚本接管
        let latest = std::panic::catch_unwind(|| latest_release(cfg.update_repo.trim()))
            .ok()
            .and_then(Result::ok)
            .and_then(|r| r.get("tag_name").and_then(|v| v.as_str()).map(str::to_string));
        let hint = match latest {
            Some(tag) => format!(
                "容器部署请在宿主机运行 ./scripts/upgrade.sh（最新版本 {tag}）"
            ),
            None => "容器部署请在宿主机运行 ./scripts/upgrade.sh".to_string(),
        };
        return Err(AppError::BadRequest(hint));
    }

    let current = env!("CARGO_PKG_VERSION").to_string();
    let repo = cfg.update_repo.trim();
    if repo.is_empty() {
        return Err(AppError::BadRequest("未配置更新仓库".into()));
    }

    let release = latest_release(repo)?;
    let tag = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("发布信息缺少版本号".into()))?
        .trim()
        .to_string();
    let latest = tag.trim_start_matches(['v', 'V']).to_string();

    if !is_newer(&latest, &current) {
        return Err(AppError::BadRequest(format!("当前已是最新版本 {current}")));
    }

    let assets = release.get("assets").cloned().unwrap_or(serde_json::Value::Null);
    let (asset_name, asset_url) = pick_asset(&assets).ok_or_else(|| {
        AppError::BadRequest(format!("{tag} 没有提供 {} 平台的可执行文件", platform_hint()))
    })?;

    let exe = std::env::current_exe()
        .map_err(|err| AppError::Internal(format!("无法定位当前程序：{err}")))?;
    let data_dir = cfg
        .update_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("./data"));
    let stage_dir: PathBuf = data_dir.join("upgrade");
    fs::create_dir_all(&stage_dir)
        .map_err(|err| AppError::Internal(format!("创建升级目录失败：{err}")))?;

    let staged = stage_dir.join(if cfg!(windows) {
        "redteam-atlas.new.exe"
    } else {
        "redteam-atlas.new"
    });
    let bytes = download(&asset_url, &staged)?;

    let pid = std::process::id();
    let script_path = stage_dir.join(if cfg!(windows) { "apply.ps1" } else { "apply.sh" });
    let script = if cfg!(windows) {
        windows_script(&staged, &exe, pid)
    } else {
        unix_script(&staged, &exe, pid)
    };
    fs::write(&script_path, script)
        .map_err(|err| AppError::Internal(format!("写入更新脚本失败：{err}")))?;

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755));
    }

    spawn_script(&script_path)?;

    // 给响应留出送达时间，然后退出，让脚本接管替换与重启
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        std::process::exit(0);
    });

    Ok(UpgradeResult {
        from: current,
        to: latest,
        asset: asset_name,
        staged: staged.display().to_string(),
        bytes,
        script: script_path.display().to_string(),
        restarting: true,
    })
}