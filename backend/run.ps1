# 启动后端（Windows）。
#
# 本机通常没装 Visual Studio 生成工具，cargo 默认的 MSVC 目标会报
# "linker `link.exe` not found"。这里固定用 GNU 工具链，等价于：
#   $env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'; cargo run
#
# 用法：
#   .\run.ps1                 启动（默认 8080）
#   .\run.ps1 --release       发布模式
#   .\run.ps1 -- --port 9090  透传参数给程序本身

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Host '未找到 rustup。请先安装 Rust：https://rustup.rs' -ForegroundColor Red
    exit 1
}

$toolchain = 'stable-x86_64-pc-windows-gnu'
if (-not (rustup toolchain list | Select-String -SimpleMatch $toolchain)) {
    Write-Host '首次运行，正在安装 GNU 工具链（约 200MB，只需一次）...'
    rustup toolchain install $toolchain --profile minimal
}

$env:RUSTUP_TOOLCHAIN = $toolchain
Write-Host "toolchain: $toolchain"
cargo run @args