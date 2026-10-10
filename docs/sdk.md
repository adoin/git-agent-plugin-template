# 插件接口说明

插件通过 v1 C ABI 与宿主交换 UTF-8 JSON，Rust 导出宏位于 vendor/git-agent-plugin-sdk。其他语言可以使用 include/git_agent_plugin.h。每个分配方负责释放自己的缓冲区。

## 请求与响应

Request 包含 action、page、context、inputs、state、store、confirmed、event。context 包含仓库 root、branch、head、remotes 和 language。HEAD 可能尚未加载；需要冻结远端 SHA 的操作由插件自己查询。

Response.ui 返回声明式 Node 树；state 更新业务状态，inputs 更新表单；store_patch 使用 JSON merge patch，null 删除字段；navigate 切换已注册页面。badge 和 repository_tab_count 返回数量。同步钩子用 state_patch 合并业务字段，不覆盖用户输入和确认。

confirmation 让宿主展示具体写入摘要，确认后以固定请求及 confirmed=true 重调同一 action。处理函数仍须检查当前权限、目标和 SHA。取消前已发生的远端写入需要刷新核实，不能盲目重试。

## 布局与驱动

basic.v1 支持 Page、Row、Column、Form、Text、Input、Select、Button、Navigate、IconText、ActionBar、Viewport、Pagination、Table、Disclosure 和 Guard。code.v1 支持 Code；diff.v1 支持 DiffBrowser。

Page.header 位于宿主标题右侧，body 使用内容区。Viewport 固定 top 与 bottom，content 使用剩余高度；Table 在内部滚动。ActionBar 左右控件共享高度。DiffBrowser 提供文件列表与带行号的原生改动查看，files 包含 path、patch、additions、deletions、truncated，labels 提供 files、empty、unavailable、truncated。

Table 列支持 width、min_width、flex、value_labels 和 value_badges。状态徽标的 tone 为 success、danger、warning、info、neutral；icon 为 check、cross、clock、play、stop、skip、dot、attention。宿主负责主题、字体和绘制。

布局深度上限 32、节点 2000、表格 500 行；消息最大 8 MiB。Code 预览裁剪为 40000 字符，单文件 diff 预览为 100000 字符并显示截断提示。完整业务数据可以由插件保留在 state。

## 页面与入口

manifest.contributes.pages 声明 id、title、toolbar_label、toolbar_icon、entry_action、badge、parent 和可选 permission。toolbar.pages 为菜单候选项，inline_pages 为直接展示入口，宿主排除重复项。commands 声明 page、action、menus，位置支持 repository、actions、tools、view。

图标为插件资源中的 SVG。locales 声明 JSON 字典；标题使用 {"key":"home"}，组件字符串使用 {{i18n:key}}。Select.options 和数据字段保存内部值，option_labels、value_labels 控制译文，用户内容保持原文。

## 权限与适用条件

任何按钮或布局可以声明 permission="store.access.increment"。父子条件共同生效，只接受布尔 true；页面和菜单也可声明 permission。manifest.action_permissions 在最终派发时检查同一 store。处理函数还须验证真实业务权限。

repository_match 可省略，省略时适用于所有仓库。remote_hosts 支持精确域名与 *.example.com；host_env 可以声明额外企业主机的环境变量。用户的全局和仓库开关在域名条件之外统一检查。

## 回调与事件

Host.call 支持 cancelled、store.get、store.patch、ai.review。store.get 的 path 采用 store. 开头点路径；store.patch 的 patch 会随工作结果按顺序提交。ai.review 需要声明 ai 能力，凭据由宿主管理。network 和 remote.write 声明业务所需能力。一次 store 快照最多 1 MiB。

订阅 repository.remote_sync.started 会与获取、拉取、推送和后台 fetch 一起异步执行，event 提供 operation_id、operation、source。开始钩子只读，不应返回 navigate 或 confirmation。completed 事件可用于需要新本地快照的处理。

插件处理不在绘制线程运行。一个动态库的调用串行，仓库状态隔离。外部命令必须限制时长和输出，定期检查取消，回收子进程。不要保存 HostApi 指针、借用缓冲区或让后台线程在调用结束后继续使用它们。

## 外部命令依赖

manifest.required_tools 可声明依赖命令名，例如 ["gh"]。宿主在后台按 PATH 检查可用性，管理页显示缺失项；安装依赖后刷新列表重新检测。此检查不执行命令，插件仍需在实际调用时处理错误和认证状态。

同一插件 ID 在本次宿主会话中复用库实例和调用门禁；关闭后库可保持驻留，但停止新调用。重启才切换 DLL 版本或删除托管文件，避免旧任务与新门禁并发进入同一原生库。
