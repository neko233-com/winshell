# WinShell

使用 Rust + GPUI 开发的原生多标签终端。Windows 发行包内置 Bash、Git 和
Unix 工具，无需另装 Git Bash；macOS 提供 Intel 和 Apple Silicon 原生应用。

## 一条命令安装

Windows，在 PowerShell 中运行：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/neko233-com/winshell/main/install.ps1)))
```

macOS，在终端中运行：

```sh
curl -fsSL https://raw.githubusercontent.com/neko233-com/winshell/main/install.sh | sh
```

脚本从 [GitHub Release](https://github.com/neko233-com/winshell/releases) 下载对应架构的包，
检查 SHA256 后安装。再次运行即可升级，升级前请关闭 WinShell。
Windows 安装在当前用户目录，并创建开始菜单入口，可通过系统“已安装的应用”卸载。
macOS 安装到 `~/Applications/WinShell.app`，命令入口为 `~/.local/bin/winshell`。

Windows 包尚未签名，macOS 包使用临时签名，尚未完成 Apple 公证。
系统应用控制可能要求用户确认；安装脚本不会关闭这些保护。
macOS 使用系统 Bash/Zsh 和工具；需要 Git 时请安装 Apple 命令行工具或其他 Git 发行版。

## 已实现

- 独立 PTY 会话、多标签、侧边栏、Shell 启动器、原生 GPU 渲染。
- Bash、PowerShell 7、Windows PowerShell、CMD，以及 macOS Bash/Zsh。
  可以配置 WSL、Nushell 等任意 Shell 可执行文件。
- 使用真实 Shell 执行命令，支持其原有的脚本、管道、重定向、别名、函数、命令替换和环境变量。
- 继承进程环境变量，并支持全局 `[env]` 和每个 Shell 的 `[shells.env]` 覆盖。
  新标签读取配置；Shell 中的 export 不会串到其他标签或修改系统注册表。
- Bash 历史提示、命令提示、Alt + 右箭头接受建议；Tab 保留 Shell 原生补全。
- 选择复制、粘贴、滚动历史、搜索、ANSI/真彩色、全屏应用备用屏幕和鼠标协议。
- 自动识别系统界面语言，支持英语、简繁中文、日语、韩语、德语、法语、西班牙语。
- 午夜薄荷、Catppuccin、Nord、明亮、自定义皮肤；切换时同时更新界面和终端配色。

## 配置和快捷键

Ctrl + 逗号打开设置，可以直接切换语言和皮肤。F6/F7 在设置中循环切换主题/语言。
Ctrl + Shift + T/W 新建/关闭标签；Ctrl + Tab 切换；Ctrl + Shift + C/V 复制/粘贴；
Ctrl + Shift + F 搜索。macOS 同时支持 Cmd + T/W/C/V/F 和 Cmd + 逗号。

Windows 配置：`%APPDATA%\winshell\config\config.toml`。
macOS 配置：`~/Library/Application Support/winshell/config.toml`。
参照 [配置示例](config.example.toml)，编辑后用 Ctrl + Shift + R 重新加载。

`winshell --doctor` 检查 Shell 和配置，`winshell --cwd PATH --shell ID` 指定启动目录和 Shell。
发生 Rust 崩溃时，在配置目录写入 `crash.log`，用于排查。

## 构建与验证

Windows 需要 Rust stable、Visual Studio C++ Build Tools 和 Windows SDK：

```powershell
.\scripts\setup-runtime.ps1
cargo build --locked
.\scripts\validate.ps1 -UI
.\scripts\package.ps1
```

macOS 需要 Rust 和 Xcode 命令行工具：`bash scripts/package-macos.sh`。
CI 验证三种平台构建、真实 Shell 和原生窗口，并验证 Windows 安装/卸载。

当前没有分屏、会话恢复、内联图片和完整无障碍树。提示层针对集成 Bash 的单行输入；
其他 Shell 使用自身的补全能力。不同系统的 IME、驱动和复杂全屏应用仍需要更广泛的兼容性验证。
这里的“支持 Shell 命令”指交给真实 Shell 执行；命令需要的软件和运行环境仍须存在。

项目采用 Apache-2.0，内置工具遵循各自许可证。详见 [第三方声明](THIRD_PARTY_NOTICES.md)。
