# Git Agent GitHub 插件与开发模板

本仓库默认项目是基于 gh 的跨平台 GitHub 插件，业务源码、版本和插件发布在这里维护。examples/minimal 保留最小示例，vendor 固定 SDK 0.1.0、ABI v1。主程序负责驱动、钩子和安装管理，不参与插件业务编译。

[English guide](README.en.md) · [SDK 接口](docs/sdk.md) · [插件发布](docs/releasing.md)

## GitHub 插件

提供 PR 列表与文件 diff、创建及管理、AI 审核草稿、人工审核、merge/squash/rebase、议题、工作流、发布和仓库概览。权限通过插件 store 驱动入口和动作检查，写操作冻结目标后再次验证权限和 SHA。界面和字典支持中文、英文。

需要安装 gh 并执行 gh auth login。只在 github.com 或 GH_HOST 对应的仓库生效。远端同步开始钩子与获取、拉取、推送及后台同步并行执行。

## 开发与测试

使用支持 Rust edition 2024 的 Rust stable、Python 3.11 或更新版本。Windows 脚本使用 PowerShell 7，MSVC 需要 C++ 构建工具。

```powershell
cargo test --workspace --locked
cargo fmt --all --check
pwsh.exe -NoLogo -NoProfile -File scripts/dev.ps1 -HostExecutable D:/workspace/git-Agent/target/debug/git-agent.exe
```

默认构建 GitHub 插件，暂存到 target/dev-plugins/github。脚本只重启它自己启动的宿主；Ctrl+C 不影响其他宿主进程。一次性验收加 -SmokeTestSeconds 5。主程序仓库的 dev.ps1 也会发现同级本仓库，可通过 -ExternalPluginProjects 显式配置联调目录。

最小示例使用 -Example minimal 或 Python 的 --example minimal，例如：

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/dev.ps1 -Example minimal -HostExecutable D:/workspace/git-Agent/target/debug/git-agent.exe
pwsh.exe -NoLogo -NoProfile -File scripts/package.ps1 -Example minimal
```

## 创建自己的插件

从本模板创建新仓库，默认用最小示例起步：

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/init.ps1 -Id my-plugin -Name "我的插件" -Example minimal
```

初始化替换根项目的示例源码、字典和资源，旧内容备份到 target/init-backup。要保留 GitHub 全部功能并改名，选择 -Example github。已有自定义插件不能通过最小示例模式覆盖；GitHub 模式只调整名称与库文件名。

修改 src、plugin.json、locales 和 assets 实现业务。版本更新同时修改根 Cargo.toml 和 plugin.json，再执行 cargo generate-lockfile。独立 SDK 快照和摘要位于 vendor，不依赖主程序目录。

## 打包与安装

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/package.ps1
```

输出 dist/github-VERSION-TARGET.gaplugin 及同名 .sha256。Linux/macOS 使用 python scripts/plugin.py package；--target 指定已安装的 Rust 目标及链接工具链。Windows、Linux 和 macOS 两种架构由 CI 分别构建。

在包含 API v1 的宿主中，通过“选项 → 插件 → 安装 / 更新插件包”导入。插件更新重启生效。宿主全局与仓库开关及域名匹配共同生效；inputs/state/权限 store 仍按仓库在会话内保存。

## 发布和预装

GitHub 插件独立打 vVERSION 标签。CI 检查 tag、Cargo 版本和 manifest 一致，发布四个平台目标的 GitHub 插件包和 SHA256 文件。最小示例也会构建作为模板回归验证，但不进入 GitHub 插件 Release。

主程序用 bundled-plugins.lock.json 固定插件仓库、版本、tag 和源码提交，再从插件 Release 获取校验过的包随安装器分发。先发布插件，再发布引用它的主程序版本；二者版本号独立。当前迁移只准备发布流程，没有创建 release tag。

许可证和 SDK 归属见 LICENSE。动态库使用当前用户权限，宿主服务声明和 store 条件不构成系统沙箱。
