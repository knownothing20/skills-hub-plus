@echo off
REM ============================================================
REM Skills Hub Plus 本地 Rust 验证(规范入口,见 AGENTS.md)
REM 只做 check / fmt / 测试编译,绝不出包;结束后自动清理编译缓存
REM 用法:双击或 cmd 执行 dev\verify-rust.bat
REM ============================================================
call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
cd /d "%~dp0..\src-tauri"

echo [1/3] cargo check ...
cargo check
if errorlevel 1 goto :fail

echo [2/3] cargo fmt --check ...
cargo fmt --all -- --check
if errorlevel 1 goto :fail

echo [3/3] cargo test --lib --no-run (仅编译测试,不在本机执行) ...
cargo test --lib --no-run
if errorlevel 1 goto :fail

echo.
echo VERIFY_OK (测试执行交给 CI 的 Linux 环境)
cd /d "%~dp0.."
rmdir /s /q src-tauri\target >nul 2>&1
echo TARGET_CACHE_CLEANED
exit /b 0

:fail
echo.
echo VERIFY_FAILED
cd /d "%~dp0.."
rmdir /s /q src-tauri\target >nul 2>&1
echo TARGET_CACHE_CLEANED
exit /b 1
