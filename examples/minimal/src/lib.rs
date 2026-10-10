use git_agent_plugin_sdk::{Host, Node, Request, Response, text_key};
use serde_json::{Value, json};
use std::collections::BTreeMap;

git_agent_plugin_sdk::export_plugin!(handle);

fn text(key: &str) -> String {
    text_key(key)
}
fn view(request: &Request, state: &Value) -> Node {
    if request.page == "preferences" {
        return Node::Page {
            header: vec![],
            body: Box::new(Node::Column {
                children: vec![
                    Node::Navigate {
                        label: text("back"),
                        page: "home".into(),
                    },
                    Node::Form {
                        id: "preferences_form".into(),
                        title: text("preferences"),
                        children: vec![
                            Node::Input {
                                id: "display_name".into(),
                                label: text("name"),
                                multiline: false,
                                width: None,
                            },
                            Node::Select {
                                id: "can_operate".into(),
                                label: text("access"),
                                options: vec!["true".into(), "false".into()],
                                option_labels: BTreeMap::from([
                                    ("true".into(), text("allowed")),
                                    ("false".into(), text("readonly")),
                                ]),
                            },
                            Node::Button {
                                label: text("save"),
                                action: "save_preferences".into(),
                            },
                        ],
                    },
                    Node::Text {
                        text: text("session_hint"),
                    },
                ],
            }),
        };
    }
    let name = request
        .inputs
        .get("display_name")
        .cloned()
        .unwrap_or_default();
    Node::Page {
        header: vec![Node::IconText {
            icon: "assets/counter.svg".into(),
            text: state["counter"].as_u64().unwrap_or(0).to_string(),
            tooltip: text("counter"),
            action: String::new(),
            selected: false,
        }],
        body: Box::new(Node::Column {
            children: vec![
                Node::Text {
                    text: format!("{} {name}", text("greeting")),
                },
                Node::Text {
                    text: format!(
                        "{}: {}",
                        text("sync_count"),
                        state["sync_count"].as_u64().unwrap_or(0)
                    ),
                },
                Node::Row {
                    children: vec![
                        Node::Button {
                            label: text("increment"),
                            action: "increment".into(),
                        }
                        .with_permission(Some("store.access.increment")),
                        Node::Navigate {
                            label: text("preferences"),
                            page: "preferences".into(),
                        },
                    ],
                },
                Node::Text {
                    text: text("access_hint"),
                },
            ],
        }),
    }
}
fn handle(mut request: Request, host: &Host) -> Result<Response, String> {
    if host.cancelled() {
        return Err("Cancelled / 已取消".into());
    }
    let mut state = json!({"counter":request.state["counter"].as_u64().unwrap_or(0),"sync_count":request.state["sync_count"].as_u64().unwrap_or(0)});
    let mut response = Response::default();
    match request.action.as_str() {
        "open" | "refresh" => {
            if request.store["access"]["increment"].is_null() {
                response.store_patch = Some(json!({"access":{"increment":true}}));
            }
            request
                .inputs
                .entry("display_name".into())
                .or_insert_with(|| "Developer".into());
            request
                .inputs
                .entry("can_operate".into())
                .or_insert_with(|| {
                    request.store["access"]["increment"]
                        .as_bool()
                        .unwrap_or(true)
                        .to_string()
                });
        }
        "increment" => {
            if request.store["access"]["increment"] != true {
                return Err("Read-only / 仅查看".into());
            }
            state["counter"] = json!(state["counter"].as_u64().unwrap_or(0) + 1);
            response.badge = state["counter"].as_u64();
        }
        "save_preferences" => {
            let allowed = request
                .inputs
                .get("can_operate")
                .is_some_and(|value| value == "true");
            let patch = json!({"access":{"increment":allowed}});
            // Host callbacks and response patches share the same scoped store.
            host.call(json!({"method":"store.patch","patch":patch}))?;
            response.store_patch = Some(patch);
            response.notice = text("saved");
        }
        "repository.remote_sync.started" => {
            state["sync_count"] = json!(state["sync_count"].as_u64().unwrap_or(0) + 1);
            response.state_patch = Some(json!({"sync_count":state["sync_count"]}));
            response.ui = Some(view(&request, &state));
            return Ok(response);
        }
        _ => return Err("Unknown example action".into()),
    }
    response.ui = Some(view(&request, &state));
    response.state = state;
    response.inputs = request.inputs;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_agent_plugin_sdk::{Buffer, Dictionary, HostApi};
    unsafe extern "C" fn service(
        _: *mut std::ffi::c_void,
        request: *const u8,
        len: usize,
    ) -> Buffer {
        let value: Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, len) }).unwrap();
        Buffer::json(&if value["method"] == "cancelled" {
            json!(false)
        } else {
            json!({})
        })
    }
    fn host() -> Host {
        Host(HostApi {
            api_version: 1,
            context: std::ptr::null_mut(),
            call: service,
            release_buffer: git_agent_plugin_sdk::release_buffer,
        })
    }
    fn request(action: &str) -> Request {
        Request {
            action: action.into(),
            page: "home".into(),
            context: Default::default(),
            inputs: BTreeMap::new(),
            state: Value::Null,
            store: Value::Null,
            confirmed: false,
            event: None,
        }
    }
    #[test]
    fn layouts_are_valid_and_dictionary_labels_are_bilingual() {
        let dictionaries = Dictionary(BTreeMap::from([
            (
                "zh".into(),
                serde_json::from_str(include_str!("../locales/zh.json")).unwrap(),
            ),
            (
                "en".into(),
                serde_json::from_str(include_str!("../locales/en.json")).unwrap(),
            ),
        ]));
        for page in ["home", "preferences"] {
            for language in ["zh", "en"] {
                let mut req = request("open");
                req.page = page.into();
                let mut view = handle(req, &host()).unwrap().ui.unwrap();
                view.validate().unwrap();
                view.localize(&dictionaries, language);
                assert!(!serde_json::to_string(&view).unwrap().contains("{{i18n:"));
            }
        }
    }
    #[test]
    fn increment_checks_authoritative_store_and_sync_preserves_counter_and_inputs() {
        assert!(handle(request("increment"), &host()).is_err());
        let mut req = request("increment");
        req.store = json!({"access":{"increment":true}});
        let response = handle(req, &host()).unwrap();
        assert_eq!(response.state["counter"], 1);
        let mut req = request("repository.remote_sync.started");
        req.state = response.state;
        req.inputs.insert("display_name".into(), "Draft".into());
        let response = handle(req, &host()).unwrap();
        assert_eq!(response.state_patch.unwrap(), json!({"sync_count":1}));
        assert!(response.inputs.is_empty());
        assert!(response.navigate.is_none());
        assert!(response.confirmation.is_none());
    }
    #[test]
    fn store_permission_is_declared_on_layout_and_manifest_dispatch() {
        let req = request("open");
        let view = handle(req, &host()).unwrap().ui.unwrap();
        assert_eq!(
            view.action_access("increment", &json!({"access":{"increment":false}})),
            Some(false)
        );
        let manifest: git_agent_plugin_sdk::Manifest =
            serde_json::from_str(include_str!("../plugin.json")).unwrap();
        assert_eq!(
            manifest.action_permissions["increment"],
            "store.access.increment"
        );
        assert!(manifest.repository_match.is_none());
    }
}
