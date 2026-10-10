use super::*;
use git_agent_plugin_sdk::{merge_store, permission_allowed, text_key};
use std::collections::BTreeMap;

pub(super) fn rule(action: &str) -> Option<&'static str> {
    match action {
        "create" => Some("store.access.create"),
        "merge" => Some("store.access.merge"),
        "close" => Some("store.access.close"),
        "reopen" => Some("store.access.reopen"),
        "ready" | "draft" => Some("store.access.update"),
        "publish_review" => Some("store.access.review"),
        "issue_create" => Some("store.access.issue_create"),
        "issue_comment" => Some("store.access.issue_comment"),
        "issue_edit" => Some("store.access.issue_update"),
        "issue_close" => Some("store.access.issue_close"),
        "issue_reopen" => Some("store.access.issue_reopen"),
        "workflow_run" => Some("store.access.workflow_run"),
        "run_rerun" => Some("store.access.run_rerun"),
        "run_cancel" => Some("store.access.run_cancel"),
        _ => None,
    }
}
pub(super) fn page_rule(page: &str) -> Option<&'static str> {
    match page {
        "pr_create" => Some("store.access.create"),
        "workflow_dispatch" => Some("store.access.workflow_run"),
        "issue_create" => Some("store.access.issue_create"),
        _ => None,
    }
}
fn target(request: &Request) -> Option<(&'static str, &str)> {
    if [
        "view",
        "diff",
        "ai_review",
        "review_view",
        "merge",
        "close",
        "reopen",
        "ready",
        "draft",
        "publish_review",
    ]
    .contains(&request.action.as_str())
        || ["pr_detail", "pr_review"].contains(&request.page.as_str())
    {
        let number = input(request, "number");
        if number.parse::<u64>().is_ok_and(|n| n > 0) {
            return Some(("pullRequest", number));
        }
    }
    if [
        "issue_view",
        "issue_comment",
        "issue_edit",
        "issue_close",
        "issue_reopen",
    ]
    .contains(&request.action.as_str())
        || request.page == "issue_detail"
    {
        let number = input(request, "issue_number");
        if number.parse::<u64>().is_ok_and(|n| n > 0) {
            return Some(("issue", number));
        }
    }
    None
}
pub(super) fn project(request: &Request, repo: &str, data: &Value) -> Value {
    let repository = &data["repository"];
    let login = data["viewer"]["login"].as_str().unwrap_or_default();
    let role = repository["viewerPermission"].as_str().unwrap_or_default();
    let writable = ["WRITE", "MAINTAIN", "ADMIN"].contains(&role);
    let readable = !login.is_empty() && repository.is_object();
    let active = readable && repository["isArchived"].as_bool() == Some(false);
    let pr = &repository["pullRequest"];
    let issue = &repository["issue"];
    let pr_active = active && pr.is_object();
    let issue_active = active && issue.is_object();
    let approve = pr_active && pr["viewerDidAuthor"].as_bool() == Some(false);
    // Creating against the repository's pushed head requires write access in this UI.
    let create = active && writable;
    let review = pr_active && (input(request, "review_kind") == "COMMENT" || approve);
    let flags = BTreeMap::from([
        ("create", create),
        (
            "merge",
            pr_active && writable && pr["state"] == "OPEN" && pr["isDraft"] == false,
        ),
        ("close", pr_active && pr["viewerCanClose"] == true),
        ("reopen", pr_active && pr["viewerCanReopen"] == true),
        ("update", pr_active && pr["viewerCanUpdate"] == true),
        (
            "manage",
            pr_active
                && (writable
                    || pr["viewerCanClose"] == true
                    || pr["viewerCanReopen"] == true
                    || pr["viewerCanUpdate"] == true),
        ),
        ("review", review),
        ("approve", approve),
        (
            "issue_create",
            active && repository["viewerCanCreateIssues"] == true,
        ),
        ("issue_comment", issue_active),
        (
            "issue_update",
            issue_active && issue["viewerCanUpdate"] == true,
        ),
        (
            "issue_close",
            issue_active && issue["viewerCanClose"] == true,
        ),
        (
            "issue_reopen",
            issue_active && issue["viewerCanReopen"] == true,
        ),
        ("workflow_run", active && writable),
        ("run_rerun", active && writable),
        ("run_cancel", active && writable),
    ]);
    json!({"account":{"login":login,"repository":repo,"role":role,"target":target(request).map(|(kind,number)|format!("{kind}:{number}"))},"access":flags})
}
pub(super) fn refresh(
    request: &mut Request,
    host: &Host,
    repo: &str,
    response: &mut Response,
) -> Result<bool, String> {
    let (hostname, name) = repo_host(repo);
    let (owner, name) = name.split_once('/').ok_or("Invalid repository")?;
    let object=target(request).map(|(kind,number)|format!("{kind}(number:{number}) {{ viewerCanClose viewerCanReopen viewerCanUpdate viewerDidAuthor {} }}",if kind=="pullRequest"{"state isDraft"}else{""})).unwrap_or_default();
    let query = format!(
        "query PluginAccess($owner:String!,$name:String!) {{ viewer {{ login }} repository(owner:$owner,name:$name) {{ viewerPermission isArchived viewerCanCreateIssues {object} }} }}"
    );
    let result = gh_json(
        request,
        host,
        &[
            "api",
            "--hostname",
            hostname,
            "graphql",
            "-f",
            &format!("query={query}"),
            "-f",
            &format!("owner={owner}"),
            "-f",
            &format!("name={name}"),
        ],
    );
    let patch = match result {
        Ok(value) if value["data"]["repository"].is_object() && value.get("errors").is_none() => {
            project(request, repo, &value["data"])
        }
        _ => project(request, repo, &Value::Null),
    };
    let changed = request.confirmed
        && request.store["account"]["login"]
            .as_str()
            .is_some_and(|previous| Some(previous) != patch["account"]["login"].as_str());
    merge_store(&mut request.store, &patch);
    response.store_patch = Some(patch);
    if changed {
        response.error = text_key("access.changed");
        return Ok(false);
    }
    if !permission_allowed(&request.store, rule(&request.action)) {
        response.error = text_key("access.denied");
        return Ok(false);
    }
    Ok(true)
}
