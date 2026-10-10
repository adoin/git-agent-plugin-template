use super::*;
fn nav(request: &Request, zh: &str, en: &str, page: &str) -> Node {
    Node::Navigate {
        label: tr(request, zh, en),
        page: page.into(),
    }
    .with_permission(access::page_rule(page))
}
fn workflow_badges(
    field: &str,
) -> std::collections::BTreeMap<String, git_agent_plugin_sdk::ValueBadge> {
    use git_agent_plugin_sdk::{BadgeIcon as I, BadgeTone as T, ValueBadge};
    let items: Vec<(&str, T, I)> = match field {
        "status" => vec![
            ("completed", T::Neutral, I::Check),
            ("in_progress", T::Info, I::Play),
            ("queued", T::Warning, I::Clock),
            ("waiting", T::Warning, I::Clock),
            ("pending", T::Warning, I::Clock),
            ("requested", T::Warning, I::Clock),
        ],
        "conclusion" => vec![
            ("success", T::Success, I::Check),
            ("failure", T::Danger, I::Cross),
            ("timed_out", T::Danger, I::Clock),
            ("startup_failure", T::Danger, I::Cross),
            ("cancelled", T::Neutral, I::Stop),
            ("skipped", T::Neutral, I::Skip),
            ("neutral", T::Neutral, I::Dot),
            ("stale", T::Warning, I::Clock),
            ("action_required", T::Warning, I::Attention),
            ("", T::Neutral, I::Clock),
        ],
        _ => Vec::new(),
    };
    items
        .into_iter()
        .map(|(value, tone, icon)| (value.into(), ValueBadge { tone, icon }))
        .collect()
}
fn table(
    request: &Request,
    id: &str,
    key: &str,
    action: &str,
    rows: Vec<Value>,
    columns: &[(&str, &str, &str)],
) -> Node {
    Node::Table {
        id: id.into(),
        row_key: key.into(),
        on_select: action.into(),
        rows,
        columns: columns
            .iter()
            .map(|(field, zh, en)| {
                let (width, min_width, flex) = match *field {
                    "number" | "issue_number" => (Some(56.0), None, false),
                    "title" | "displayTitle" | "name" => (None, Some(240.0), true),
                    "kind" | "isDraft" | "isPrerelease" => (Some(72.0), None, false),
                    "author" => (Some(112.0), None, false),
                    "updated_at" | "updatedAt" | "publishedAt" => (Some(180.0), None, false),
                    "run_id" | "workflow_id" => (Some(116.0), None, false),
                    _ => (Some(136.0), None, false),
                };
                Column {
                    field: (*field).into(),
                    label: tr(request, zh, en),
                    value_badges: if id == "runs" {
                        workflow_badges(field)
                    } else {
                        Default::default()
                    },
                    value_labels: if [
                        "kind",
                        "state",
                        "status",
                        "conclusion",
                        "isDraft",
                        "isPrerelease",
                    ]
                    .contains(field)
                    {
                        locales::dictionary().0["en"]
                            .keys()
                            .filter_map(|key| {
                                key.strip_prefix("value.").map(|value| {
                                    (value.to_owned(), git_agent_plugin_sdk::text_key(key))
                                })
                            })
                            .collect()
                    } else {
                        Default::default()
                    },
                    width,
                    min_width,
                    flex,
                }
            })
            .collect(),
    }
}
fn pager(request: &Request, state: &Value, prefix: &str) -> Node {
    let key = |field: &str| {
        if prefix == "pr" {
            field.to_owned()
        } else {
            format!("{prefix}_{field}")
        }
    };
    let pages = state[key("pages")].as_u64().unwrap_or(1).max(1);
    Node::Pagination {
        id: format!("{prefix}_pagination"),
        page: state[key("page")].as_u64().unwrap_or(1).clamp(1, pages),
        page_count: pages,
        total: state[key("total")].as_u64().unwrap_or(0),
        page_size: page_size_for(request, prefix),
        page_sizes: vec![10, 20, 30, 50],
        page_key: format!("{prefix}_page"),
        size_key: format!("{prefix}_page_size"),
        on_page: if prefix == "pr" {
            "set_page".into()
        } else {
            format!("{prefix}_set_page")
        },
        on_page_size: if prefix == "pr" {
            "page_size_changed".into()
        } else {
            format!("{prefix}_page_size_changed")
        },
        labels: ["total", "size", "unit", "previous", "next", "page"]
            .into_iter()
            .map(|key| {
                (
                    key.into(),
                    git_agent_plugin_sdk::text_key(&format!("pagination.{key}")),
                )
            })
            .collect(),
    }
}
fn lookup(request: &Request, id: &str, zh: &str, en: &str) -> Node {
    let mut node = field(request, id, zh, en, false);
    if let Node::Input { width, .. } = &mut node {
        *width = Some(88.0);
    }
    node
}
pub(super) fn page(request: &Request, state: &Value) -> Node {
    let mut children = if matches!(
        request.page.as_str(),
        "prs" | "issues" | "actions" | "workflows" | "pr_detail"
    ) {
        Vec::new()
    } else {
        vec![Node::Row {
            children: vec![Node::Text {
                text: format!(
                    "{}: {}",
                    tr(request, "仓库", "Repository"),
                    input(request, "repo")
                ),
            }],
        }]
    };
    let pr = &state["detail"];
    match request.page.as_str() {
        "prs" => {
            let mut number = field(request, "number", "PR 编号", "PR number", false);
            if let Node::Input { width, .. } = &mut number {
                *width = Some(88.0);
            }
            children.extend(vec![
                Node::ActionBar {
                    left: vec![
                        field(request, "query", "搜索", "Search", false),
                        select(
                            request,
                            "filter",
                            "状态",
                            "State",
                            &["open", "pending", "draft", "closed", "merged", "all"],
                        ),
                        button(request, "刷新", "Refresh", "refresh"),
                        nav(request, "创建 PR", "Create PR", "pr_create"),
                    ],
                    right: vec![
                        number,
                        button(request, "查看 PR", "View PR", "view"),
                        button(request, "当前分支 PR", "Current branch PR", "current_pr"),
                    ],
                    right_width: 400.0,
                },
                table(
                    request,
                    "prs",
                    "number",
                    "view",
                    state["rows"].as_array().cloned().unwrap_or_default(),
                    &[
                        ("number", "编号", "#"),
                        ("title", "标题", "Title"),
                        ("kind", "类型", "Type"),
                        ("author", "作者", "Author"),
                        ("updated_at", "更新时间", "Updated"),
                    ],
                ),
                pager(request, state, "pr"),
            ]);
        }
        "pr_detail" => {
            let mut top = vec![Node::ActionBar {
                left: vec![
                    nav(request, "返回 PR 列表", "Back to pull requests", "prs"),
                    button(request, "刷新详情", "Refresh details", "view"),
                    button(request, "浏览器打开", "Open in browser", "pr_web"),
                ],
                right: vec![
                    button(request, "AI 审核", "AI review", "ai_review"),
                    nav(request, "人工审核与草稿", "Review and draft", "pr_review"),
                ],
                right_width: 280.0,
            }];
            if !pr.is_null() {
                top.push(Node::Text {
                    text: format!(
                        "#{} {} · {} → {} · {} · {}",
                        pr["number"],
                        text(pr, "title"),
                        text(pr, "headRefName"),
                        text(pr, "baseRefName"),
                        locales::value_label(&text(pr, "state"))
                            .unwrap_or_else(|| text(pr, "state")),
                        locales::value_label(&text(pr, "mergeStateStatus"))
                            .unwrap_or_else(|| text(pr, "mergeStateStatus"))
                    ),
                });
                top.push(Node::Disclosure {
                    title: tr(request, "描述与检查", "Description and checks"),
                    text: format!(
                        "{}\n\n{}: {}\n{}: {}",
                        text(pr, "body"),
                        tr(request, "审核决定", "Review decision"),
                        locales::value_label(&text(pr, "reviewDecision"))
                            .unwrap_or_else(|| text(pr, "reviewDecision")),
                        tr(request, "检查", "Checks"),
                        pr["statusCheckRollup"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|check| format!(
                                "{}: {}",
                                check["name"]
                                    .as_str()
                                    .or_else(|| check["context"].as_str())
                                    .unwrap_or_default(),
                                {
                                    let value = check["conclusion"]
                                        .as_str()
                                        .filter(|v| !v.is_empty())
                                        .or_else(|| check["status"].as_str())
                                        .or_else(|| check["state"].as_str())
                                        .unwrap_or_default();
                                    locales::value_label(value).unwrap_or_else(|| value.into())
                                }
                            ))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    ),
                });
            }
            children.push(Node::Column { children: top });
            children.push(Node::DiffBrowser {
                id: format!("pr_files_{}", pr["number"].as_u64().unwrap_or(0)),
                files: if state["diff_head"] == pr["headRefOid"]
                    && state["diff_base"] == pr["baseRefOid"]
                {
                    serde_json::from_value(state["diff_files"].clone())
                        .unwrap_or_else(|_| files::collect(pr, ""))
                } else {
                    files::collect(pr, "")
                },
                labels: [
                    ("files", "diff.files"),
                    ("empty", "diff.empty"),
                    ("unavailable", "diff.unavailable"),
                    ("truncated", "diff.truncated"),
                ]
                .into_iter()
                .map(|(key, value)| (key.into(), git_agent_plugin_sdk::text_key(value)))
                .collect(),
            });
            children.push(
                Node::Row {
                    children: vec![
                        select(
                            request,
                            "merge_method",
                            "合并方式",
                            "Merge method",
                            &["squash", "merge", "rebase"],
                        ),
                        button(request, "预览合并", "Preview merge", "merge"),
                        button(request, "关闭 PR", "Close PR", "close"),
                        button(request, "重开 PR", "Reopen PR", "reopen"),
                        if pr["isDraft"].as_bool() == Some(true) {
                            button(request, "标记为可审核", "Mark ready", "ready")
                        } else {
                            button(request, "转为草稿", "Convert to draft", "draft")
                        },
                    ],
                }
                .with_permission(Some("store.access.manage")),
            );
        }
        "pr_review" => children.extend(vec![
            nav(request, "返回 PR 详情", "Back to PR details", "pr_detail"),
            Node::Row {
                children: vec![
                    field(request, "number", "PR 编号", "PR number", false),
                    button(request, "读取审核对象", "Load review target", "review_view"),
                    button(request, "AI 审核", "AI review", "ai_review"),
                ],
            },
            Node::Text {
                text: format!("{} · {}", text(pr, "title"), text(pr, "headRefOid")),
            },
            Node::Form {
                id: "review_form".into(),
                title: tr(request, "审核草稿", "Review draft"),
                children: vec![
                    field(request, "review_body", "审核内容", "Review body", true),
                    select(
                        request,
                        "review_kind",
                        "审核方式",
                        "Review kind",
                        &["COMMENT", "APPROVE", "REQUEST_CHANGES"],
                    ),
                    button(
                        request,
                        "预览并发布审核",
                        "Preview and publish review",
                        "publish_review",
                    ),
                ],
            },
        ]),
        "pr_create" => children.extend(vec![
            nav(request, "返回 PR 列表", "Back to pull requests", "prs"),
            Node::Form {
                id: "create_form".into(),
                title: tr(request, "分支须已推送", "Push the branch first"),
                children: vec![
                    field(request, "title", "标题", "Title", false),
                    field(request, "body", "正文", "Body", true),
                    Node::Row {
                        children: vec![
                            field(request, "base", "目标分支", "Base branch", false),
                            field(
                                request,
                                "head",
                                "来源分支或 owner:branch",
                                "Head branch or owner:branch",
                                false,
                            ),
                            select(request, "draft", "草稿", "Draft", &["yes", "no"]),
                        ],
                    },
                    button(request, "预览创建", "Preview creation", "create"),
                ],
            },
        ]),
        "issues" => children.extend(vec![
            Node::ActionBar {
                left: vec![
                    field(request, "issue_query", "搜索", "Search", false),
                    select(
                        request,
                        "issue_filter",
                        "状态",
                        "State",
                        &["open", "closed", "all"],
                    ),
                    button(request, "刷新", "Refresh", "refresh"),
                    nav(request, "创建议题", "Create issue", "issue_create"),
                ],
                right: vec![
                    lookup(request, "issue_number", "议题编号", "Issue number"),
                    button(request, "查看议题", "View issue", "issue_view"),
                ],
                right_width: 400.0,
            },
            table(
                request,
                "issues",
                "issue_number",
                "issue_view",
                state["issues"].as_array().cloned().unwrap_or_default(),
                &[
                    ("issue_number", "编号", "#"),
                    ("title", "标题", "Title"),
                    ("state", "状态", "State"),
                    ("author", "作者", "Author"),
                    ("updatedAt", "更新时间", "Updated"),
                ],
            ),
            pager(request, state, "issue"),
        ]),
        "issue_detail" => {
            children.extend(vec![
                nav(request, "返回议题", "Back to issues", "issues"),
                Node::Row {
                    children: vec![
                        field(request, "issue_number", "议题编号", "Issue number", false),
                        button(request, "刷新详情", "Refresh details", "issue_view"),
                        button(request, "浏览器打开", "Open in browser", "issue_web"),
                    ],
                },
            ]);
            let issue = &state["issue_detail"];
            if !issue.is_null() {
                children.push(Node::Text {
                    text: format!(
                        "#{} {} · {}\n{}",
                        issue["number"],
                        text(issue, "title"),
                        locales::value_label(&text(issue, "state"))
                            .unwrap_or_else(|| text(issue, "state")),
                        text(issue, "body")
                    ),
                });
                children.push(Node::Code {
                    text: serde_json::to_string_pretty(&issue["comments"]).unwrap_or_default(),
                });
            }
            children.extend(vec![
                field(request, "issue_comment", "评论", "Comment", true),
                button(request, "预览发布评论", "Preview comment", "issue_comment"),
                Node::Row {
                    children: vec![
                        button(request, "关闭议题", "Close issue", "issue_close"),
                        button(request, "重开议题", "Reopen issue", "issue_reopen"),
                    ],
                },
                Node::Form {
                    id: "issue_edit".into(),
                    title: tr(request, "编辑议题", "Edit issue"),
                    children: vec![
                        field(request, "issue_title", "标题", "Title", false),
                        field(request, "issue_body", "正文", "Body", true),
                        button(request, "预览保存", "Preview save", "issue_edit"),
                    ],
                },
            ]);
        }
        "issue_create" => children.extend(vec![
            nav(request, "返回议题", "Back to issues", "issues"),
            Node::Form {
                id: "new_issue".into(),
                title: tr(request, "新建议题", "New issue"),
                children: vec![
                    field(request, "new_issue_title", "标题", "Title", false),
                    field(request, "new_issue_body", "正文", "Body", true),
                    field(
                        request,
                        "new_issue_labels",
                        "标签（逗号分隔）",
                        "Labels (comma separated)",
                        false,
                    ),
                    field(
                        request,
                        "new_issue_assignees",
                        "负责人（逗号分隔）",
                        "Assignees (comma separated)",
                        false,
                    ),
                    button(request, "预览创建", "Preview creation", "issue_create"),
                ],
            },
        ]),
        "actions" => children.extend(vec![
            Node::ActionBar {
                left: vec![
                    field(request, "run_branch", "分支筛选", "Branch filter", false),
                    select(
                        request,
                        "run_status",
                        "状态",
                        "Status",
                        &[
                            "all",
                            "queued",
                            "in_progress",
                            "completed",
                            "failure",
                            "success",
                            "cancelled",
                        ],
                    ),
                    button(request, "刷新", "Refresh", "refresh"),
                ],
                right: vec![
                    lookup(request, "run_id", "运行 ID", "Run ID"),
                    button(request, "查看运行", "View run", "run_view"),
                    nav(request, "工作流定义", "Workflow definitions", "workflows"),
                ],
                right_width: 400.0,
            },
            table(
                request,
                "runs",
                "run_id",
                "run_view",
                state["runs"].as_array().cloned().unwrap_or_default(),
                &[
                    ("run_id", "运行 ID", "Run ID"),
                    ("displayTitle", "标题", "Title"),
                    ("workflowName", "工作流", "Workflow"),
                    ("headBranch", "分支", "Branch"),
                    ("status", "状态", "Status"),
                    ("conclusion", "结果", "Result"),
                ],
            ),
            pager(request, state, "run"),
        ]),
        "workflows" => children.extend(vec![
            Node::ActionBar {
                left: vec![
                    nav(request, "返回运行记录", "Back to runs", "actions"),
                    button(request, "刷新", "Refresh", "refresh"),
                ],
                right: vec![nav(
                    request,
                    "手动触发工作流",
                    "Dispatch workflow",
                    "workflow_dispatch",
                )],
                right_width: 400.0,
            },
            table(
                request,
                "workflows",
                "workflow_id",
                "select_workflow",
                state["workflows"].as_array().cloned().unwrap_or_default(),
                &[
                    ("workflow_id", "工作流 ID", "Workflow ID"),
                    ("name", "名称", "Name"),
                    ("state", "状态", "State"),
                ],
            ),
            pager(request, state, "workflow"),
        ]),
        "workflow_dispatch" => children.extend(vec![
            nav(
                request,
                "返回工作流定义",
                "Back to workflow definitions",
                "workflows",
            ),
            Node::Form {
                id: "workflow_dispatch".into(),
                title: tr(request, "手动触发工作流", "Dispatch workflow"),
                children: vec![
                    field(
                        request,
                        "workflow_id",
                        "工作流 ID 或文件名",
                        "Workflow ID or file",
                        false,
                    ),
                    field(
                        request,
                        "workflow_ref",
                        "分支或标签",
                        "Branch or tag",
                        false,
                    ),
                    field(
                        request,
                        "workflow_inputs",
                        "输入参数 JSON",
                        "Inputs JSON",
                        true,
                    ),
                    button(request, "预览触发", "Preview dispatch", "workflow_run"),
                ],
            },
        ]),
        "run_detail" => children.extend(vec![
            nav(request, "返回工作流", "Back to actions", "actions"),
            Node::Row {
                children: vec![
                    field(request, "run_id", "运行 ID", "Run ID", false),
                    button(request, "刷新运行", "Refresh run", "run_view"),
                    button(request, "浏览器打开", "Open in browser", "run_web"),
                ],
            },
            Node::Code {
                text: serde_json::to_string_pretty(&state["run_detail"]).unwrap_or_default(),
            },
            Node::Row {
                children: vec![
                    button(request, "失败日志", "Failed job logs", "run_logs"),
                    select(
                        request,
                        "rerun_kind",
                        "重跑范围",
                        "Rerun scope",
                        &["failed", "all"],
                    ),
                    button(request, "预览重跑", "Preview rerun", "run_rerun"),
                    button(request, "预览取消", "Preview cancel", "run_cancel"),
                ],
            },
            Node::Code {
                text: state["run_logs"].as_str().unwrap_or_default().into(),
            },
        ]),
        "releases" => children.extend(vec![
            button(request, "刷新发布", "Refresh releases", "refresh"),
            table(
                request,
                "releases",
                "tagName",
                "release_view",
                state["releases"].as_array().cloned().unwrap_or_default(),
                &[
                    ("tagName", "标签", "Tag"),
                    ("name", "名称", "Name"),
                    ("isDraft", "草稿", "Draft"),
                    ("isPrerelease", "预发布", "Prerelease"),
                    ("publishedAt", "发布时间", "Published"),
                ],
            ),
            Node::Row {
                children: vec![
                    field(request, "tagName", "标签", "Tag", false),
                    button(request, "查看 Release", "View release", "release_view"),
                    button(request, "浏览器打开", "Open in browser", "release_web"),
                ],
            },
            Node::Code {
                text: serde_json::to_string_pretty(&state["release_detail"]).unwrap_or_default(),
            },
        ]),
        "repository" => children.extend(vec![
            button(request, "刷新仓库信息", "Refresh repository", "refresh"),
            button(
                request,
                "浏览器打开仓库",
                "Open repository in browser",
                "repo_web",
            ),
            Node::Code {
                text: serde_json::to_string_pretty(&state["repository"]).unwrap_or_default(),
            },
        ]),
        _ => children.push(Node::Text {
            text: tr(request, "未知页面", "Unknown page"),
        }),
    }
    let body = if matches!(
        request.page.as_str(),
        "prs" | "issues" | "actions" | "workflows" | "pr_detail"
    ) {
        let mut nodes = children.into_iter();
        Node::Viewport {
            top: Box::new(nodes.next().unwrap()),
            content: Box::new(nodes.next().unwrap()),
            bottom: Box::new(nodes.next().unwrap()),
        }
    } else {
        Node::Column { children }
    };
    if request.page == "prs" {
        let metrics = [
            (
                "pending",
                "assets/pull-request.svg",
                "待合并（打开的非草稿 PR）",
                "Pending (open, non-draft PRs)",
            ),
            ("draft", "assets/draft.svg", "草稿 PR", "Draft PRs"),
            (
                "open",
                "assets/open-prs.svg",
                "打开的 PR 总数",
                "All open PRs",
            ),
        ];
        let header = metrics
            .into_iter()
            .map(|(key, icon, zh, en)| Node::IconText {
                icon: icon.into(),
                text: state["counts"][key]
                    .as_u64()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "…".into()),
                tooltip: tr(request, zh, en),
                action: format!("filter_{key}"),
                selected: input(request, "filter") == key,
            })
            .collect();
        Node::Page {
            header,
            body: Box::new(body),
        }
    } else {
        Node::Page {
            header: Vec::new(),
            body: Box::new(body),
        }
    }
}
