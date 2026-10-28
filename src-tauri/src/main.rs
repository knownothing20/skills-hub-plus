// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "windows")]
    {
        // 关键防护：在 Windows 多显卡（独显+核显）或虚拟显示器/远程桌面环境下，
        // WebView2 GPU 硬件合成容易发生 DirectComposition 表面丢失并挂起，导致窗口白屏且事件循环锁死。
        let existing = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
        if !existing.contains("--disable-features=CalculateNativeWinOcclusion") {
            let extra = "--disable-features=CalculateNativeWinOcclusion --disable-gpu-compositing";
            let args = if existing.is_empty() {
                extra.to_string()
            } else {
                format!("{existing} {extra}")
            };
            std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
        }
    }

    app_lib::run();
}
