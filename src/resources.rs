use super::*;
fn id(request: &Request, key: &str) -> Result<String, String> {
    let value = input(request, key);
    if value.parse::<u64>().is_ok_and(|n| n > 0) {
        Ok(value.into())
    } else {
        Err(format!("Select a valid {key} / 请选择有效编号"))
    }
}
fn issue(request: &Request, host: &Host, repo: &str, number: &str) -> Result<Value, String> {
    gh_json(
        request,
        host,
        &[
            "issue",
            "view",
            number,
            "--repo",
            repo,
            "--json",
            "number,title,body,state,author,labels,assignees,comments,url",
        ],
    )
}
pub(super) fn list_keys(page: &str) -> Option<(&'static str, &'static str)> {
    match page {
        "issues" => Some(("issue", "issues")),
        "actions" => Some(("run", "runs")),
        "workflows" => Some(("workflow", "workflows")),
        _ => None,
    }
}
pub(super) fn refresh_list(
    request: &mut Request,
    host: &Host,
    repo: &str,
    state: &mut Value,
) -> Result<(), String> {
    let (prefix, rows_key) = list_keys(&request.page).ok_or("Unknown resource list")?;
    let size = page_size_for(request, prefix);
    let page_key = format!("{prefix}_page");
    let mut page = if request.action == "repository.remote_sync.started" {
        input(request, &page_key)
            .parse::<u64>()
            .ok()
            .unwrap_or_else(|| state[&page_key].as_u64().unwrap_or(1))
    } else if request.action == format!("{prefix}_set_page") {
        input(request, &page_key).parse::<u64>().unwrap_or(1)
    } else {
        1
    };
    page = page.max(1);
    let (hostname, name) = repo_host(repo);
    for attempt in 0..3 {
        let mut args = strings(&["api", "--hostname", hostname, "-X", "GET"]);
        let endpoint = match request.page.as_str() {
            "issues" => "search/issues".to_owned(),
            "actions" => format!("repos/{name}/actions/runs"),
            _ => format!("repos/{name}/actions/workflows"),
        };
        args.push(endpoint);
        args.extend(strings(&[
            "-f",
            &format!("per_page={size}"),
            "-f",
            &format!("page={page}"),
        ]));
        if request.page == "issues" {
            let filter = match input(request, "issue_filter") {
                "closed" => "is:closed",
                "all" => "",
                _ => "is:open",
            };
            args.extend(strings(&[
                "-f",
                &format!(
                    "q=repo:{name} is:issue {filter} {}",
                    input(request, "issue_query")
                ),
                "-f",
                "sort=updated",
                "-f",
                "order=desc",
            ]));
        } else if request.page == "actions" {
            for (key, binding) in [("status", "run_status"), ("branch", "run_branch")] {
                let value = input(request, binding);
                if !value.is_empty() && value != "all" {
                    args.extend(strings(&["-f", &format!("{key}={value}")]));
                }
            }
        }
        let result: Value = serde_json::from_str(&gh(host, &request.context.root, &args, None)?)
            .map_err(|_| "Invalid resource list")?;
        let total = result["total_count"]
            .as_u64()
            .ok_or("Missing resource total count")?;
        let accessible = if request.page == "issues"
            || (request.page == "actions"
                && (input(request, "run_status") != "all"
                    || !input(request, "run_branch").is_empty()))
        {
            total.min(1000)
        } else {
            total
        };
        let pages = accessible.div_ceil(size).max(1);
        if page > pages {
            page = if attempt == 0 { pages } else { 1 };
            continue;
        }
        let array = match request.page.as_str() {
            "issues" => "items",
            "actions" => "workflow_runs",
            _ => "workflows",
        };
        let rows=result[array].as_array().ok_or("Invalid resource rows")?.iter().map(|row| match request.page.as_str() {
            "issues"=>json!({"issue_number":row["number"],"title":row["title"],"state":row["state"],"author":row["user"]["login"],"updatedAt":row["updated_at"]}),
            "actions"=>json!({"run_id":row["id"],"displayTitle":row["display_title"],"workflowName":row["name"],"headBranch":row["head_branch"],"status":row["status"],"conclusion":row["conclusion"],"createdAt":row["created_at"],"url":row["html_url"]}),
            _=>{let mut value=row.clone();value["workflow_id"]=row["id"].clone();value},
        }).collect::<Vec<_>>();
        state[rows_key] = json!(rows);
        state[&page_key] = json!(page);
        state[format!("{prefix}_pages")] = json!(pages);
        state[format!("{prefix}_total")] = json!(total);
        request.inputs.insert(page_key, page.to_string());
        request
            .inputs
            .insert(format!("{prefix}_page_size"), size.to_string());
        return Ok(());
    }
    Err("Resource pagination changed repeatedly; refresh again".into())
}
pub(super) fn handle(
    request: &mut Request,
    host: &Host,
    repo: &str,
    state: &mut Value,
    response: &mut Response,
) -> Result<bool, String> {
    match request.action.as_str() {
        "refresh"
        | "issue_set_page"
        | "issue_page_size_changed"
        | "run_set_page"
        | "run_page_size_changed"
        | "workflow_set_page"
        | "workflow_page_size_changed"
            if list_keys(&request.page).is_some() =>
        {
            refresh_list(request, host, repo, state)?;
        }
        "issue_view" => {
            let number = id(request, "issue_number")?;
            let value = issue(request, host, repo, &number)?;
            if state["issue_detail"]["number"] != value["number"] {
                request
                    .inputs
                    .insert("issue_title".into(), text(&value, "title"));
                request
                    .inputs
                    .insert("issue_body".into(), text(&value, "body"));
                request.inputs.insert("issue_comment".into(), String::new());
            }
            state["issue_detail"] = value;
            response.navigate = Some("issue_detail".into());
        }
        "issue_create" | "issue_comment" | "issue_edit" | "issue_close" | "issue_reopen" => {
            let action = request.action.clone();
            let number = if action == "issue_create" {
                String::new()
            } else {
                id(request, "issue_number")?
            };
            let title = if action == "issue_create" {
                input(request, "new_issue_title")
            } else {
                input(request, "issue_title")
            };
            let body = match action.as_str() {
                "issue_create" => input(request, "new_issue_body"),
                "issue_comment" => input(request, "issue_comment"),
                _ => input(request, "issue_body"),
            };
            if ["issue_create", "issue_edit"].contains(&action.as_str()) && title.trim().is_empty()
            {
                return Err("Issue title is required".into());
            }
            if action == "issue_comment" && body.trim().is_empty() {
                return Err("Comment is required".into());
            }
            if !request.confirmed {
                response.confirmation = Some(format!(
                    "{action} {repo} #{number}\n{title}\n{body}\nLabels: {}\nAssignees: {}",
                    input(request, "new_issue_labels"),
                    input(request, "new_issue_assignees")
                ));
            } else {
                let mut args = match action.as_str() {
                    "issue_create" => strings(&[
                        "issue",
                        "create",
                        "--repo",
                        repo,
                        "--title",
                        title,
                        "--body-file",
                        "-",
                    ]),
                    "issue_edit" => strings(&[
                        "issue",
                        "edit",
                        &number,
                        "--repo",
                        repo,
                        "--title",
                        title,
                        "--body-file",
                        "-",
                    ]),
                    "issue_comment" => strings(&[
                        "issue",
                        "comment",
                        &number,
                        "--repo",
                        repo,
                        "--body-file",
                        "-",
                    ]),
                    "issue_close" => strings(&["issue", "close", &number, "--repo", repo]),
                    _ => strings(&["issue", "reopen", &number, "--repo", repo]),
                };
                if action == "issue_create" {
                    for (key, flag) in [
                        ("new_issue_labels", "--label"),
                        ("new_issue_assignees", "--assignee"),
                    ] {
                        for value in input(request, key)
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            args.extend(strings(&[flag, value]));
                        }
                    }
                }
                response.notice = gh(
                    host,
                    &request.context.root,
                    &args,
                    if ["issue_create", "issue_edit", "issue_comment"].contains(&action.as_str()) {
                        Some(body)
                    } else {
                        None
                    },
                )?;
                if action != "issue_create" {
                    state["issue_detail"]=issue(request,host,repo,&number).map_err(|e|format!("Remote write completed, but refresh failed: {e}. Refresh before retrying."))?;
                    request.inputs.insert("issue_comment".into(), String::new());
                }
            }
        }
        "select_workflow" => {
            response.navigate = Some("workflow_dispatch".into());
        }
        "run_view" => {
            let number = id(request, "run_id")?;
            state["run_detail"] = gh_json(
                request,
                host,
                &[
                    "run",
                    "view",
                    &number,
                    "--repo",
                    repo,
                    "--json",
                    "databaseId,displayTitle,workflowName,headBranch,status,conclusion,jobs,url",
                ],
            )?;
            state["run_logs"] = Value::Null;
            response.navigate = Some("run_detail".into());
        }
        "run_logs" => {
            let number = id(request, "run_id")?;
            state["run_logs"] = json!(run(
                request,
                host,
                &["run", "view", &number, "--repo", repo, "--log-failed"]
            )?);
        }
        "run_rerun" | "run_cancel" => {
            let number = id(request, "run_id")?;
            let action = request.action.clone();
            if !request.confirmed {
                let current = gh_json(
                    request,
                    host,
                    &[
                        "run",
                        "view",
                        &number,
                        "--repo",
                        repo,
                        "--json",
                        "databaseId,displayTitle,status,conclusion,attempt,headSha",
                    ],
                )?;
                state["pending_run"] = current.clone();
                response.confirmation = Some(format!(
                    "{action} {repo} run {number}\nScope: {}\n{}",
                    input(request, "rerun_kind"),
                    serde_json::to_string_pretty(&current).unwrap_or_default()
                ));
            } else {
                let current = gh_json(
                    request,
                    host,
                    &[
                        "run",
                        "view",
                        &number,
                        "--repo",
                        repo,
                        "--json",
                        "databaseId,displayTitle,status,conclusion,attempt,headSha",
                    ],
                )?;
                if current["headSha"] != state["pending_run"]["headSha"]
                    || current["attempt"] != state["pending_run"]["attempt"]
                    || current["databaseId"] != state["pending_run"]["databaseId"]
                {
                    return Err("Workflow run changed; refresh and preview again".into());
                }
                let mut args = strings(&[
                    "run",
                    if action == "run_rerun" {
                        "rerun"
                    } else {
                        "cancel"
                    },
                    &number,
                    "--repo",
                    repo,
                ]);
                if action == "run_rerun" && input(request, "rerun_kind") == "failed" {
                    args.push("--failed".into());
                }
                response.notice = gh(host, &request.context.root, &args, None)?;
                response.notice.push_str(&tr(
                    request,
                    "\n请求已提交，请刷新查看状态。",
                    "\nRequest submitted. Refresh to check status.",
                ));
            }
        }
        "workflow_run" => {
            let workflow = input(request, "workflow_id");
            let reference = input(request, "workflow_ref");
            if workflow.is_empty() || reference.is_empty() {
                return Err("Workflow and ref are required".into());
            }
            let body = if input(request, "workflow_inputs").trim().is_empty() {
                "{}"
            } else {
                input(request, "workflow_inputs")
            };
            let inputs: Value =
                serde_json::from_str(body).map_err(|_| "Workflow inputs must be JSON")?;
            if !inputs.is_object()
                || inputs
                    .as_object()
                    .is_some_and(|o| o.values().any(|v| !v.is_string()))
            {
                return Err("Workflow inputs must be an object of strings".into());
            }
            if !request.confirmed {
                response.confirmation = Some(format!(
                    "Dispatch {repo}\nWorkflow: {workflow}\nRef: {reference}\n{body}"
                ));
            } else {
                response.notice = gh(
                    host,
                    &request.context.root,
                    &strings(&[
                        "workflow", "run", workflow, "--repo", repo, "--ref", reference, "--json",
                    ]),
                    Some(body),
                )?;
                response.notice.push_str(&tr(
                    request,
                    "\n工作流已触发，请刷新运行列表。",
                    "\nWorkflow dispatched. Refresh runs.",
                ));
            }
        }
        "refresh" if request.page == "releases" => {
            state["releases"] = gh_json(
                request,
                host,
                &[
                    "release",
                    "list",
                    "--repo",
                    repo,
                    "--limit",
                    "100",
                    "--json",
                    "tagName,name,isDraft,isPrerelease,publishedAt",
                ],
            )?;
        }
        "release_view" => {
            let tag = input(request, "tagName");
            if tag.is_empty() {
                return Err("Select a release tag".into());
            }
            state["release_detail"] = gh_json(
                request,
                host,
                &[
                    "release",
                    "view",
                    tag,
                    "--repo",
                    repo,
                    "--json",
                    "tagName,name,body,isDraft,isPrerelease,publishedAt,assets,url",
                ],
            )?;
        }
        "refresh" if request.page == "repository" => {
            state["repository"] = gh_json(
                request,
                host,
                &[
                    "repo",
                    "view",
                    repo,
                    "--json",
                    "nameWithOwner,description,url,visibility,defaultBranchRef,viewerPermission,isFork",
                ],
            )?;
        }
        "repo_web" => {
            run(request, host, &["repo", "view", repo, "--web"])?;
        }
        "issue_web" => {
            let number = id(request, "issue_number")?;
            run(
                request,
                host,
                &["issue", "view", &number, "--repo", repo, "--web"],
            )?;
        }
        "run_web" => {
            let number = id(request, "run_id")?;
            run(
                request,
                host,
                &["run", "view", &number, "--repo", repo, "--web"],
            )?;
        }
        "release_web" => {
            let tag = input(request, "tagName");
            if tag.is_empty() {
                return Err("Select a release".into());
            }
            run(
                request,
                host,
                &["release", "view", tag, "--repo", repo, "--web"],
            )?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
