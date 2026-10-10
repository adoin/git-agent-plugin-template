# GitHub plugin 0.1.0

Released / 发布日期：2026-10-10

GitHub plugin for Git Agent API v1. Requires GitHub CLI (`gh`) and an authenticated GitHub account.

Git Agent API v1 的 GitHub 插件，需要安装并登录 GitHub CLI（`gh`）。

Download the package matching your operating system and architecture, then import it through Options → Plugins. Plugin updates activate after restarting Git Agent. SHA256 sidecars accompany each package.

下载对应系统和架构的插件包，在“选项 → 插件”导入；更新后重启生效。每个包附带 SHA256 校验文件。

The source, permissions model and bilingual developer guides are included in this repository. GitHub plugin versions are independent of the Git Agent application version.

插件源码、权限模型和双语开发指南均在本仓库中维护。插件版本与 Git Agent 主程序版本独立。

This initial release provides PR lists and file diffs, AI review drafts, permission-controlled PR management, issues, workflow runs, releases and repository summaries. Data refresh joins the application's remote synchronization hooks. The repository includes a minimal starter and development and packaging scripts.

首个版本提供 PR 列表与文件差异、AI 审核草稿、按权限开放的 PR 管理、议题、工作流运行、发布和仓库概览。数据刷新接入主程序远端同步钩子。仓库同时提供最小示例及开发、打包脚本。
