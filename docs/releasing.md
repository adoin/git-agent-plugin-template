# GitHub 插件独立发布

GitHub 业务只在本仓库维护，主程序仅引用发布包。插件包的版本与主程序版本独立。

## 版本和验证

更新根 Cargo.toml、plugin.json 的 version，执行 cargo generate-lockfile、cargo fmt --all --check、cargo test --workspace --locked。最小示例和固定 SDK 一起测试。执行 python scripts/check-release.py vVERSION 检查拟发布标签，更新 docs/release-notes.md 描述这次变更。

提交并推送代码后，在该提交上创建 vVERSION 标签并推送。标签触发 CI，生成四种目标包以及 SHA256：x86_64-pc-windows-msvc、x86_64-unknown-linux-gnu、aarch64-apple-darwin、x86_64-apple-darwin。所有构建成功后，发布 GitHub Release。普通分支提交只测试和生成 Actions 产物，不发布 Release。

失败的构建不产生完整 Release。不要移动已有公共 tag；发布修复版本时提升插件版本号。源码、标签和 Release 资产应对应同一提交。

## 主程序引用

主程序 bundled-plugins.lock.json 固定 repository、tag、version、revision。prepare-bundled-plugins.py 通过已认证 gh 解析 tag，确认源码提交，下载指定平台包与 SHA256，并校验包内身份和版本。主程序的安装器把这些包随应用分发。

首次启动由通用安装管理导入随包插件。已有更高版本不降级；用户关闭的开关保留；已经卸载的随包插件不会在以后启动时重新安装。新版随包插件只在启动阶段安排更新，读取原生库前完成切换。

发布顺序是插件 Release 成功后，再提交主程序的版本锁定及版本升级，最后发布主程序。迁移阶段只配置流程，不提前打 tag。

## English summary

Keep the default GitHub Cargo/manifest versions identical and test the complete workspace. Push an immutable vVERSION tag after updating release notes. CI publishes GitHub packages and SHA256 sidecars for Windows/Linux x86_64 and both macOS architectures; branch builds publish Actions artifacts only.

The host pins tag/version/revision, verifies the commit and package checksum, and bundles the selected assets. Publish the plugin first, then the host version that pins it. User-disabled/uninstalled plugins stay disabled/uninstalled, and newer user-installed versions are preserved.
