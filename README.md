# Git Agent 插件模板

这个仓库可以独立开发、测试和打包 Git Agent 动态库插件。示例包含两个页面、中英字典、同步钩子和声明式权限；界面由宿主固定驱动绘制。SDK 0.1.0 随仓库固定，ABI 为 v1，无需把模板放进 Git Agent 源码目录。

[English guide](README.en.md) · [SDK 接口说明](docs/sdk.md)

## 准备环境

安装支持 Rust 2024 edition 的 Rust stable、Python 3.10 或更新版本。Windows 脚本使用 PowerShell 7，Rust MSVC 构建需要 C++ 构建工具。

联调需要包含 API v1 和插件管理功能的 Git Agent 构建。本模板在当前开发分支的 1.4.8 构建上验证。

## 创建自己的插件

从模板创建新仓库，在仓库根目录执行：

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/init.ps1 -Id my-plugin -Name "我的插件"
```

脚本修改 Cargo 包名、库名、插件 ID 和平台库文件名，并更新 Cargo.lock。继续修改：

- `src/lib.rs`：处理 action，返回 UI 和状态。
- `plugin.json`：页面、菜单、驱动、能力、钩子和可选域名条件。
- `locales/zh.json`、`locales/en.json`：显示文案。
- `assets`：功能 SVG 图标。

同一台主机上的插件 ID 应唯一。更新版本时同时修改 Cargo.toml 和 plugin.json 的 version，执行 `cargo generate-lockfile`。

## 测试和联调

```powershell
cargo test --workspace --locked
cargo fmt --all --check
pwsh.exe -NoLogo -NoProfile -File scripts/dev.ps1 -HostExecutable D:/workspace/git-Agent/target/debug/git-agent.exe
```

开发脚本构建并暂存到 `target/dev-plugins/<id>`，通过 GIT_AGENT_PLUGIN_DIR 启动宿主，监听代码、字典和资源。修改后只停止脚本自己启动的宿主进程，再构建和重启。Ctrl+C 同样只结束这个进程。宿主已有的托管插件仍可加载，开发目录的同 ID 插件优先；全局和仓库开关依然生效。

不传 HostExecutable 时只构建、暂存和监听。一次性检查可以加 `-SmokeTestSeconds 5`。Linux 和 macOS 使用 `python scripts/plugin.py dev --host /path/to/git-agent`。

## 打包和安装

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/package.ps1
```

Linux 和 macOS 使用 `python scripts/plugin.py package`。输出位于 `dist/<id>-<version>-<target>.gaplugin`。指定目标可以传 `-Target` 或 `--target`，对应 Rust target 和链接工具链需要已安装。

在宿主的“选项 → 插件”中选择“安装 / 更新插件包”。新插件安装后加载；同 ID 的更高版本安排在重启后启用。全局开关控制所有仓库；“仓库设置 → 插件”允许继承或为当前仓库关闭、启用。全局关闭时，仓库级启用不会越过全局开关。可选域名条件仍独立生效。

卸载停止新调用，已有任务协作收尾，文件在重启时清除。用户开关和仓库设置保留。开发目录插件由开发者管理源码，不能用卸载按钮删除源码。

插件包是 ZIP，根目录包含 plugin.json、package.json、本平台动态库和所需字典及资源。package.json 声明目标平台架构、API 和最低宿主版本。安装校验不执行动态库；启用加载时才验证真实 ABI 入口。动态库运行在宿主进程，宿主服务声明与 store 权限并不提供系统级沙箱。

## 示例行为

示例默认在所有仓库生效，不声明 repository_match。打开页面可以计数，在示例设置中切换演示权限；关闭权限时按钮隐藏，派发和处理函数也检查相同权限。获取、拉取、推送及后台同步启动时，示例参与异步钩子，并更新参与次数，不调用外部服务。

inputs、state 和权限 store 按插件和仓库在会话内隔离。示例的名称、计数和演示权限不跨重启保存；宿主管理的安装记录、全局开关和仓库覆盖会持久化。

## SDK 和持续集成

`vendor/git-agent-plugin-sdk` 是 SDK 0.1.0 的独立快照，`vendor/sdk-version.json` 记录 API 和源文件摘要。升级 SDK 时替换这个目录、更新摘要及版本约束，再完整测试；不自动跟随主项目的未固定分支。

CI 在 Windows、Linux、macOS 上运行格式和工作区测试，并上传当前 runner 实际架构的插件包。不同平台的库需分别构建。SDK 与模板的许可和归属见随附 LICENSE。
