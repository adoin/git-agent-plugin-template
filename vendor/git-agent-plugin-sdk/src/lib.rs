//! v1 ABI: no Rust-owned object crosses the library boundary.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::c_void;

pub const API_VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;

#[repr(C)]
pub struct Buffer {
    pub ptr: *mut u8,
    pub len: usize,
}
impl Buffer {
    pub fn json(value: &impl Serialize) -> Self {
        let bytes = serde_json::to_vec(value)
            .expect("serializable plugin message")
            .into_boxed_slice();
        let len = bytes.len();
        Self {
            ptr: Box::into_raw(bytes) as *mut u8,
            len,
        }
    }
}
/// # Safety
/// Buffer must have been allocated by this instance of the SDK and released exactly once.
pub unsafe extern "C" fn release_buffer(buffer: Buffer) {
    if !buffer.ptr.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                buffer.ptr, buffer.len,
            )));
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostApi {
    pub api_version: u32,
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> Buffer,
    pub release_buffer: unsafe extern "C" fn(Buffer),
}
pub struct Host(pub HostApi);
impl Host {
    pub fn call(&self, request: Value) -> Result<Value, String> {
        let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        let buffer = unsafe { (self.0.call)(self.0.context, bytes.as_ptr(), bytes.len()) };
        let result = if buffer.ptr.is_null() || buffer.len > MAX_MESSAGE {
            Err("Invalid host response buffer".into())
        } else {
            serde_json::from_slice::<Value>(unsafe {
                std::slice::from_raw_parts(buffer.ptr, buffer.len)
            })
            .map_err(|e| e.to_string())
        };
        unsafe {
            (self.0.release_buffer)(buffer);
        }
        let value = result?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            Err(error.to_owned())
        } else {
            Ok(value)
        }
    }
    pub fn cancelled(&self) -> bool {
        self.call(serde_json::json!({"method":"cancelled"}))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginApi {
    pub api_version: u32,
    pub struct_size: u32,
    pub invoke: unsafe extern "C" fn(*const u8, usize, HostApi) -> Buffer,
    pub release_buffer: unsafe extern "C" fn(Buffer),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub library: std::collections::BTreeMap<String, String>,
    pub drivers: Vec<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub repository_match: Option<RepositoryMatch>,
    #[serde(default)]
    pub action_permissions: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub badge_labels: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub locales: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub contributes: Contributions,
}
/// A plugin with a repository condition needs at least one matching fetch remote.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RepositoryMatch {
    #[serde(default)]
    pub remote_hosts: Vec<String>,
    #[serde(default)]
    pub host_env: Option<String>,
}
fn normalized_host(authority: &str) -> Option<String> {
    let host = if authority.starts_with('[') {
        let end = authority.find(']')?;
        let suffix = &authority[end + 1..];
        if !suffix.is_empty()
            && !suffix
                .strip_prefix(':')
                .is_some_and(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        &authority[..=end]
    } else {
        let (host, port) = authority
            .split_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)));
        if port.is_some_and(|port| port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit())) {
            return None;
        }
        host
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
    {
        return None;
    }
    Some(host)
}
pub fn remote_host(url: &str) -> Option<String> {
    let url = url.trim();
    let authority = if let Some((scheme, rest)) = url.split_once("://") {
        if !["http", "https", "ssh", "git"].contains(&scheme.to_ascii_lowercase().as_str()) {
            return None;
        }
        rest.split(['/', '?', '#']).next()?
    } else {
        let (authority, path) = url.split_once(':')?;
        if authority.len() == 1 || authority.contains(['/', '\\']) || path.is_empty() {
            return None;
        }
        authority
    };
    normalized_host(authority.rsplit('@').next()?)
}
impl RepositoryMatch {
    pub fn validate(&self) -> Result<(), String> {
        if self.remote_hosts.len() > 64
            || self.remote_hosts.iter().any(|pattern| {
                let host = pattern.strip_prefix("*.").unwrap_or(pattern);
                normalized_host(host).as_deref()
                    != Some(host.to_ascii_lowercase().trim_end_matches('.'))
                    || host.contains(':')
            })
        {
            return Err("Invalid plugin remote-host pattern".into());
        }
        if self.host_env.as_ref().is_some_and(|name| {
            name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        }) {
            return Err("Invalid plugin host environment variable".into());
        }
        Ok(())
    }
    pub fn matches(&self, remotes: &[Remote], extra_host: Option<&str>) -> bool {
        if self.remote_hosts.is_empty() && self.host_env.is_none() {
            return true;
        }
        let extra = extra_host.and_then(normalized_host);
        remotes
            .iter()
            .filter_map(|remote| remote_host(&remote.url))
            .any(|host| {
                extra.as_deref() == Some(host.as_str())
                    || self.remote_hosts.iter().any(|pattern| {
                        let pattern = pattern.to_ascii_lowercase();
                        let pattern = pattern.trim_end_matches('.');
                        if let Some(suffix) = pattern.strip_prefix("*.") {
                            host.len() > suffix.len() + 1 && host.ends_with(&format!(".{suffix}"))
                        } else {
                            host == pattern
                        }
                    })
            })
    }
}
impl Manifest {
    pub fn matches_repository(&self, remotes: &[Remote], extra_host: Option<&str>) -> bool {
        self.repository_match
            .as_ref()
            .is_none_or(|condition| condition.matches(remotes, extra_host))
    }
}
pub type LocalizedText = std::collections::BTreeMap<String, String>;
/// Plugin-owned translation keys; dictionaries are cached by the host at load time.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Dictionary(pub std::collections::BTreeMap<String, LocalizedText>);

pub fn text_key(key: &str) -> String {
    format!("{{{{i18n:{key}}}}}")
}

impl Dictionary {
    pub fn get(&self, key: &str, language: &str) -> &str {
        let base = language.split(['-', '_']).next().unwrap_or(language);
        self.0
            .get(language)
            .or_else(|| self.0.get(base))
            .and_then(|entries| entries.get(key))
            .or_else(|| self.0.get("en").and_then(|entries| entries.get(key)))
            .or_else(|| self.0.values().find_map(|entries| entries.get(key)))
            .map(String::as_str)
            .unwrap_or("")
    }
    pub fn text(&self, text: &str, language: &str) -> String {
        let mut result = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("{{i18n:") {
            result.push_str(&rest[..start]);
            let token = &rest[start + 7..];
            let Some(end) = token.find("}}") else {
                result.push_str(&rest[start..]);
                return result;
            };
            let key = &token[..end];
            let value = self.get(key, language);
            result.push_str(if value.is_empty() { key } else { value });
            rest = &token[end + 2..];
        }
        result.push_str(rest);
        result
    }
    pub fn localized(&self, text: &LocalizedText, language: &str) -> String {
        if let Some(key) = text.get("key") {
            let value = self.get(key, language);
            return if value.is_empty() {
                key.clone()
            } else {
                value.to_owned()
            };
        }
        let base = language.split(['-', '_']).next().unwrap_or(language);
        let value = text
            .get(language)
            .or_else(|| text.get(base))
            .or_else(|| text.get("en"))
            .or_else(|| text.values().next())
            .map(String::as_str)
            .unwrap_or_default();
        self.text(value, language)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contributions {
    #[serde(default)]
    pub icons: Vec<String>,
    #[serde(default)]
    pub pages: Vec<PageDefinition>,
    #[serde(default)]
    pub commands: Vec<CommandDefinition>,
    #[serde(default)]
    pub toolbar: Option<ToolbarContribution>,
    #[serde(default)]
    pub repository_tab_badge: Option<RepositoryTabBadgeContribution>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryTabBadgeContribution {
    pub icon: String,
    pub label: LocalizedText,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageDefinition {
    pub id: String,
    pub title: LocalizedText,
    #[serde(default)]
    pub parent: String,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default)]
    pub toolbar_label: LocalizedText,
    #[serde(default)]
    pub toolbar_icon: String,
    #[serde(default)]
    pub badge: bool,
    #[serde(default = "default_entry_action")]
    pub entry_action: String,
}
fn default_entry_action() -> String {
    "open".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandDefinition {
    #[serde(default)]
    pub permission: Option<String>,
    pub id: String,
    pub title: LocalizedText,
    pub page: String,
    pub action: String,
    #[serde(default)]
    pub menus: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolbarContribution {
    #[serde(default)]
    pub icon: String,
    pub pages: Vec<String>,
    #[serde(default)]
    pub inline_pages: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RepositoryContext {
    pub root: String,
    pub branch: String,
    pub head: String,
    pub language: String,
    pub remotes: Vec<Remote>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub action: String,
    #[serde(default)]
    pub page: String,
    pub context: RepositoryContext,
    #[serde(default)]
    pub inputs: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub store: Value,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub event: Option<RemoteSyncEvent>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteSyncEvent {
    pub operation_id: u64,
    pub operation: String,
    pub source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub ui: Option<Node>,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub state_patch: Option<Value>,
    #[serde(default)]
    pub store_patch: Option<Value>,
    #[serde(default)]
    pub store: Option<Value>,
    #[serde(default)]
    pub store_updates: Vec<Value>,
    #[serde(default)]
    pub inputs: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub badge: Option<u64>,
    #[serde(default)]
    pub badge_tooltip: String,
    #[serde(default)]
    pub repository_tab_count: Option<u64>,
    #[serde(default)]
    pub notice: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub confirmation: Option<String>,
    #[serde(default)]
    pub navigate: Option<String>,
}
pub fn store_value<'a>(store: &'a Value, path: &str) -> Option<&'a Value> {
    let mut value = store;
    for key in path.strip_prefix("store.")?.split('.') {
        if key.is_empty() {
            return None;
        }
        value = value.as_object()?.get(key)?;
    }
    Some(value)
}
pub fn valid_store_path(path: &str) -> bool {
    path.strip_prefix("store.").is_some_and(|path| {
        !path.is_empty()
            && path.split('.').all(|key| {
                !key.is_empty()
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
    })
}
pub fn permission_allowed(store: &Value, path: Option<&str>) -> bool {
    path.is_none_or(|path| store_value(store, path).and_then(Value::as_bool) == Some(true))
}
/// JSON merge-patch: objects merge recursively and null removes a field.
pub fn merge_store(store: &mut Value, patch: &Value) {
    if let Some(fields) = patch.as_object() {
        if !store.is_object() {
            *store = serde_json::json!({});
        }
        for (key, value) in fields {
            if value.is_null() {
                store.as_object_mut().unwrap().remove(key);
            } else {
                merge_store(
                    store
                        .as_object_mut()
                        .unwrap()
                        .entry(key.clone())
                        .or_insert(Value::Null),
                    value,
                );
            }
        }
    } else {
        *store = patch.clone();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeTone {
    Success,
    Danger,
    Warning,
    Info,
    Neutral,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeIcon {
    Check,
    Cross,
    Clock,
    Play,
    Stop,
    Skip,
    Dot,
    Attention,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValueBadge {
    pub tone: BadgeTone,
    pub icon: BadgeIcon,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Column {
    pub field: String,
    pub label: String,
    #[serde(default)]
    pub value_labels: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub value_badges: std::collections::BTreeMap<String, ValueBadge>,
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub min_width: Option<f32>,
    #[serde(default)]
    pub flex: bool,
}
/// A file supplied by the plugin; the host never fetches data while painting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffFile {
    pub path: String,
    pub patch: String,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    #[serde(default)]
    pub truncated: bool,
}
macro_rules! node_schema {
    ($($variants:tt)*) => {
        #[derive(Clone, Debug, Serialize)]
        #[serde(tag="type",rename_all="snake_case")]
        pub enum Node {$($variants)*}
        #[derive(Deserialize)]
        #[serde(remote="Node",tag="type",rename_all="snake_case")]
        enum NodeDecoder {$($variants)*}
    }
}
node_schema! {
    Guard {permission:String,child:Box<Node>},
    Page {
        header: Vec<Node>,
        body: Box<Node>,
    },
    IconText {
        icon: String,
        text: String,
        tooltip: String,
        #[serde(default)]
        action: String,
        #[serde(default)]
        selected: bool,
    },
    ActionBar {
        left: Vec<Node>,
        right: Vec<Node>,
        right_width: f32,
    },
    Viewport {
        top: Box<Node>,
        content: Box<Node>,
        bottom: Box<Node>,
    },
    Pagination {
        id: String,
        page: u64,
        page_count: u64,
        total: u64,
        page_size: u64,
        page_sizes: Vec<u64>,
        page_key: String,
        size_key: String,
        on_page: String,
        on_page_size: String,
        labels: LocalizedText,
    },
    Navigate {
        label: String,
        page: String,
    },
    Row {
        children: Vec<Node>,
    },
    Column {
        children: Vec<Node>,
    },
    Form {
        id: String,
        title: String,
        children: Vec<Node>,
    },
    Text {
        text: String,
    },
    Input {
        id: String,
        label: String,
        #[serde(default)]
        multiline: bool,
        #[serde(default)]
        width: Option<f32>,
    },
    Select {
        id: String,
        label: String,
        options: Vec<String>,
        #[serde(default)]
        option_labels: std::collections::BTreeMap<String, String>,
    },
    Button {
        label: String,
        action: String,
    },
    Table {
        id: String,
        columns: Vec<Column>,
        rows: Vec<Value>,
        row_key: String,
        on_select: String,
    },
    Disclosure { title: String, text: String },
    DiffBrowser {
        id: String,
        files: Vec<DiffFile>,
        labels: LocalizedText,
    },
    Code {
        text: String,
    },
}
impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut value = Value::deserialize(deserializer)?;
        let permission = if value["type"] != "guard" {
            value
                .as_object_mut()
                .and_then(|object| object.remove("permission"))
        } else {
            None
        };
        let node = NodeDecoder::deserialize(value).map_err(D::Error::custom)?;
        match permission {
            None | Some(Value::Null) => Ok(node),
            Some(Value::String(permission)) => Ok(Node::Guard {
                permission,
                child: Box::new(node),
            }),
            _ => Err(D::Error::custom("permission must be a store path")),
        }
    }
}
impl Node {
    pub fn with_permission(self, path: Option<&str>) -> Self {
        if let Some(permission) = path {
            Self::Guard {
                permission: permission.into(),
                child: Box::new(self),
            }
        } else {
            self
        }
    }
    pub fn action_access(&self, target: &str, store: &Value) -> Option<bool> {
        fn walk(node: &Node, target: &str, store: &Value, allowed: bool, matches: &mut Vec<bool>) {
            match node {
                Node::Guard { permission, child } => walk(
                    child,
                    target,
                    store,
                    allowed && permission_allowed(store, Some(permission)),
                    matches,
                ),
                Node::Button { action, .. } | Node::IconText { action, .. } if action == target => {
                    matches.push(allowed)
                }
                Node::Navigate { page, .. } if target == format!("@page:{page}") => {
                    matches.push(allowed)
                }
                Node::Page { header, body } => {
                    for node in header {
                        walk(node, target, store, allowed, matches);
                    }
                    walk(body, target, store, allowed, matches);
                }
                Node::Row { children }
                | Node::Column { children }
                | Node::Form { children, .. } => {
                    for node in children {
                        walk(node, target, store, allowed, matches);
                    }
                }
                Node::ActionBar { left, right, .. } => {
                    for node in left.iter().chain(right) {
                        walk(node, target, store, allowed, matches);
                    }
                }
                Node::Viewport {
                    top,
                    content,
                    bottom,
                } => {
                    for node in [top, content, bottom] {
                        walk(node, target, store, allowed, matches);
                    }
                }
                _ => {}
            }
        }
        let mut matches = Vec::new();
        walk(self, target, store, true, &mut matches);
        (!matches.is_empty()).then(|| matches.into_iter().any(|allowed| allowed))
    }
    pub fn localize(&mut self, dictionary: &Dictionary, language: &str) {
        let resolve = |value: &mut String| *value = dictionary.text(value, language);
        match self {
            Node::Guard { child, .. } => child.localize(dictionary, language),
            Node::Page { header, body } => {
                for node in header {
                    node.localize(dictionary, language);
                }
                body.localize(dictionary, language);
            }
            Node::Viewport {
                top,
                content,
                bottom,
            } => {
                for node in [top, content, bottom] {
                    node.localize(dictionary, language);
                }
            }
            Node::Pagination { labels, .. } | Node::DiffBrowser { labels, .. } => {
                for label in labels.values_mut() {
                    resolve(label);
                }
            }
            Node::ActionBar { left, right, .. } => {
                for node in left.iter_mut().chain(right) {
                    node.localize(dictionary, language);
                }
            }
            Node::Row { children } | Node::Column { children } => {
                for node in children {
                    node.localize(dictionary, language);
                }
            }
            Node::Form {
                title, children, ..
            } => {
                resolve(title);
                for node in children {
                    node.localize(dictionary, language);
                }
            }
            Node::IconText { text, tooltip, .. } => {
                resolve(text);
                resolve(tooltip);
            }
            Node::Text { text } => resolve(text),
            Node::Disclosure { title, text } => {
                resolve(title);
                resolve(text);
            }
            Node::Input { label, .. }
            | Node::Button { label, .. }
            | Node::Navigate { label, .. } => resolve(label),
            Node::Select {
                label,
                option_labels,
                ..
            } => {
                resolve(label);
                for value in option_labels.values_mut() {
                    resolve(value);
                }
            }
            Node::Table { columns, .. } => {
                for column in columns {
                    resolve(&mut column.label);
                    for value in column.value_labels.values_mut() {
                        resolve(value);
                    }
                }
            }
            Node::Code { .. } => {}
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        fn walk(
            node: &Node,
            depth: usize,
            count: &mut usize,
            ids: &mut std::collections::HashSet<String>,
        ) -> Result<(), String> {
            *count += 1;
            if depth > 32 || *count > 2000 {
                return Err("Plugin layout exceeded limits".into());
            }
            let id = match node {
                Node::Form { id, .. }
                | Node::Input { id, .. }
                | Node::Select { id, .. }
                | Node::Table { id, .. }
                | Node::Pagination { id, .. }
                | Node::DiffBrowser { id, .. } => Some(id),
                _ => None,
            };
            if let Some(id) = id {
                if id.is_empty() || !ids.insert(id.clone()) {
                    return Err("Duplicate or empty component ID".into());
                }
            }
            match node {
                Node::Guard { permission, child } => {
                    if !valid_store_path(permission) {
                        return Err("Invalid permission store path".into());
                    }
                    walk(child, depth + 1, count, ids)?;
                }

                Node::DiffBrowser { files, .. } => {
                    let mut paths = std::collections::HashSet::new();
                    if files.len() > 3000
                        || files
                            .iter()
                            .any(|file| file.path.is_empty() || !paths.insert(&file.path))
                    {
                        return Err("Invalid diff file list".into());
                    }
                }
                Node::Input {
                    width: Some(width), ..
                } if !width.is_finite() || !(24.0..=1200.0).contains(width) => {
                    return Err("Invalid input width".into());
                }
                Node::Page { header, body } => {
                    for child in header {
                        walk(child, depth + 1, count, ids)?;
                    }
                    walk(body, depth + 1, count, ids)?;
                }
                Node::Viewport {
                    top,
                    content,
                    bottom,
                } => {
                    for child in [top, content, bottom] {
                        walk(child, depth + 1, count, ids)?;
                    }
                }
                Node::Pagination {
                    page,
                    page_count,
                    page_size,
                    page_sizes,
                    page_key,
                    size_key,
                    on_page,
                    on_page_size,
                    ..
                } => {
                    if *page_count == 0
                        || *page == 0
                        || page > page_count
                        || page_sizes.is_empty()
                        || page_sizes.len() > 10
                        || !page_sizes.contains(page_size)
                        || page_sizes.iter().any(|size| *size == 0 || *size > 500)
                        || [page_key, size_key, on_page, on_page_size]
                            .iter()
                            .any(|value| value.is_empty())
                    {
                        return Err("Invalid pagination configuration".into());
                    }
                }
                Node::ActionBar {
                    left,
                    right,
                    right_width,
                } => {
                    if !right_width.is_finite() || !(100.0..=1000.0).contains(right_width) {
                        return Err("Invalid action bar width".into());
                    }
                    for child in left.iter().chain(right) {
                        walk(child, depth + 1, count, ids)?;
                    }
                }
                Node::Row { children }
                | Node::Column { children }
                | Node::Form { children, .. } => {
                    for child in children {
                        walk(child, depth + 1, count, ids)?;
                    }
                }
                Node::Table { columns, rows, .. } => {
                    if rows.len() > 500 {
                        return Err("Plugin table exceeded row limit".into());
                    }
                    if columns.is_empty() || columns.len() > 32 {
                        return Err("Plugin table must have 1 to 32 columns".into());
                    }
                    let mut fields = std::collections::HashSet::new();
                    for column in columns {
                        if column.field.is_empty() || !fields.insert(&column.field) {
                            return Err("Duplicate or empty table field".into());
                        }
                        if [column.width, column.min_width]
                            .into_iter()
                            .flatten()
                            .any(|width| !width.is_finite() || !(24.0..=2000.0).contains(&width))
                        {
                            return Err("Invalid plugin table column width".into());
                        }
                        if column.flex && column.width.is_some() {
                            return Err(
                                "Flexible table column cannot specify an exact width".into()
                            );
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        }
        walk(self, 0, &mut 0, &mut std::collections::HashSet::new())
    }
}

/// Export a stateless handler; all plugin-owned buffers are freed inside the DLL.
#[macro_export]
macro_rules! export_plugin {
    ($handler:path) => {
        unsafe extern "C" fn invoke(
            ptr: *const u8,
            len: usize,
            host: $crate::HostApi,
        ) -> $crate::Buffer {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if ptr.is_null()
                    || len > $crate::MAX_MESSAGE
                    || host.api_version != $crate::API_VERSION
                {
                    return Err("Invalid plugin request".to_owned());
                }
                let request = serde_json::from_slice::<$crate::Request>(unsafe {
                    std::slice::from_raw_parts(ptr, len)
                })
                .map_err(|e| e.to_string())?;
                $handler(request, &$crate::Host(host))
            }));
            let response = match result {
                Ok(Ok(value)) => value,
                Ok(Err(error)) => $crate::Response {
                    error,
                    ..Default::default()
                },
                Err(_) => $crate::Response {
                    error: "Plugin panicked".into(),
                    ..Default::default()
                },
            };
            $crate::Buffer::json(&response)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn git_agent_plugin_entry_v1() -> *const $crate::PluginApi {
            static API: $crate::PluginApi = $crate::PluginApi {
                api_version: $crate::API_VERSION,
                struct_size: std::mem::size_of::<$crate::PluginApi>() as u32,
                invoke,
                release_buffer: $crate::release_buffer,
            };
            &API
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diff_browser_validates_file_identity_and_localizes_only_ui_labels() {
        let mut node:Node=serde_json::from_value(serde_json::json!({"type":"diff_browser","id":"files","files":[{"path":"{{i18n:label}}.rs","patch":"+{{i18n:label}}"}],"labels":{"files":"{{i18n:label}}"}})).unwrap();
        let dictionary = Dictionary(std::collections::BTreeMap::from([(
            "en".into(),
            std::collections::BTreeMap::from([("label".into(), "Changed files".into())]),
        )]));
        node.localize(&dictionary, "en");
        node.validate().unwrap();
        let Node::DiffBrowser { files, labels, .. } = &mut node else {
            panic!()
        };
        assert_eq!(labels["files"], "Changed files");
        assert_eq!(files[0].path, "{{i18n:label}}.rs");
        assert_eq!(files[0].patch, "+{{i18n:label}}");
        files.push(files[0].clone());
        assert!(node.validate().is_err());
    }
    #[test]
    fn rejects_duplicate_ids_and_deep_layouts() {
        let field = Node::Input {
            id: "x".into(),
            label: "X".into(),
            multiline: false,
            width: None,
        };
        assert!(
            Node::Row {
                children: vec![field.clone(), field]
            }
            .validate()
            .is_err()
        );
        let mut node = Node::Text { text: "ok".into() };
        for _ in 0..34 {
            node = Node::Column {
                children: vec![node],
            };
        }
        assert!(node.validate().is_err());
    }
    #[test]
    fn dictionary_fallback_and_ui_labels_preserve_wire_values_and_user_content() {
        let dictionary: Dictionary = serde_json::from_value(serde_json::json!({
            "zh":{"title":"标题","open":"打开","draft":"草稿"},
            "en":{"title":"Title","open":"Open","draft":"Draft","missing_zh":"English fallback"}
        }))
        .unwrap();
        assert_eq!(dictionary.text("{{i18n:title}} · 3", "zh-CN"), "标题 · 3");
        assert_eq!(
            dictionary.text("{{i18n:missing_zh}}", "zh"),
            "English fallback"
        );
        assert_eq!(dictionary.text("{{i18n:unknown}}", "zh"), "unknown");
        assert_eq!(
            dictionary.localized(
                &std::collections::BTreeMap::from([("key".into(), "title".into())]),
                "en"
            ),
            "Title"
        );
        let raw = serde_json::json!({"type":"column","children":[
            {"type":"select","id":"filter","label":"{{i18n:title}}","options":["open"],"option_labels":{"open":"{{i18n:open}}"}},
            {"type":"table","id":"prs","columns":[{"field":"kind","label":"{{i18n:title}}","value_labels":{"Draft":"{{i18n:draft}}"}}],"rows":[{"number":1,"title":"User title {{i18n:title}}","kind":"Draft"}],"row_key":"number","on_select":"view"},
            {"type":"code","text":"Code {{i18n:title}}"}
        ]});
        let mut view: Node = serde_json::from_value(raw.clone()).unwrap();
        view.localize(&dictionary, "zh");
        let translated = serde_json::to_value(view).unwrap();
        assert_eq!(
            translated["children"][0]["options"],
            raw["children"][0]["options"]
        );
        assert_eq!(translated["children"][0]["option_labels"]["open"], "打开");
        assert_eq!(
            translated["children"][1]["rows"],
            raw["children"][1]["rows"]
        );
        assert_eq!(
            translated["children"][1]["columns"][0]["value_labels"]["Draft"],
            "草稿"
        );
        assert_eq!(translated["children"][2], raw["children"][2]);
    }
    #[test]
    fn pagination_validates_bounds_and_localizes_labels() {
        let value = serde_json::json!({"type":"pagination","id":"pager","page":1,"page_count":2,"total":30,"page_size":20,"page_sizes":[10,20,30,50],"page_key":"page","size_key":"page_size","on_page":"set_page","on_page_size":"set_size","labels":{"size":"{{i18n:size}}"}});
        let mut node: Node = serde_json::from_value(value.clone()).unwrap();
        node.validate().unwrap();
        let dictionary: Dictionary = serde_json::from_value(
            serde_json::json!({"zh":{"size":"每页"},"en":{"size":"Per page"}}),
        )
        .unwrap();
        node.localize(&dictionary, "zh");
        assert!(matches!(&node,Node::Pagination{labels,..} if labels["size"]=="每页"));
        for (key, bad) in [
            ("page", serde_json::json!(0)),
            ("page", serde_json::json!(3)),
            ("page_size", serde_json::json!(15)),
            ("page_count", serde_json::json!(0)),
        ] {
            let mut invalid = value.clone();
            invalid[key] = bad;
            assert!(
                serde_json::from_value::<Node>(invalid)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
    }
    #[test]
    fn repository_domain_conditions_match_real_remote_hosts_not_paths_or_userinfo() {
        let condition = RepositoryMatch {
            remote_hosts: vec!["github.com".into(), "*.github.example.com".into()],
            host_env: Some("GH_HOST".into()),
        };
        condition.validate().unwrap();
        for url in [
            "git@github.com:owner/repo.git",
            "https://GitHub.COM/owner/repo",
            "ssh://git@github.com:22/owner/repo",
            "git://github.com/owner/repo",
            "https://user:password@github.com:443/owner/repo",
            "https://github.com./owner/repo",
            "https://team.github.example.com/o/r",
        ] {
            assert!(
                condition.matches(
                    &[Remote {
                        name: "origin".into(),
                        url: url.into()
                    }],
                    None
                ),
                "{url}"
            );
        }
        for url in [
            "git@gitlab.com:team/repo.git",
            "https://github.com.evil.example/o/r",
            "https://github.com@evil.example/o/r",
            "https://evil.example/github.com/o/r",
            "file://github.com/o/r",
            "C:/github.com/repo",
            "../repo",
            "https://github.example.com/o/r",
        ] {
            assert!(
                !condition.matches(
                    &[Remote {
                        name: "origin".into(),
                        url: url.into()
                    }],
                    None
                ),
                "{url}"
            );
        }
        assert!(!condition.matches(&[], Some("github.com")));
        assert!(condition.matches(
            &[Remote {
                name: "origin".into(),
                url: "git@github.enterprise.com:o/r".into()
            }],
            Some("github.enterprise.com")
        ));
        assert!(
            RepositoryMatch {
                remote_hosts: vec!["*".into()],
                host_env: None
            }
            .validate()
            .is_err()
        );
        assert!(
            RepositoryMatch {
                remote_hosts: vec!["evil.*.com".into()],
                host_env: None
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn omitted_or_empty_domain_conditions_apply_to_every_repository() {
        let mut manifest: Manifest =
            serde_json::from_value(serde_json::json!({"id":"example","name":"Example","version":"0.1.0","api_version":1,"library":{},"drivers":[],"capabilities":[]})).unwrap();
        manifest.repository_match = None;
        assert!(manifest.matches_repository(&[], None));
        let remotes = vec![Remote {
            name: "origin".into(),
            url: "git@gitlab.example.com:team/repo".into(),
        }];
        assert!(manifest.matches_repository(&remotes, None));
        manifest.repository_match = Some(RepositoryMatch::default());
        assert!(manifest.matches_repository(&[], None));
        assert!(manifest.matches_repository(&remotes, None));
    }
    #[test]
    fn flat_permissions_cover_buttons_and_layouts_and_merge_patch_removes_values() {
        let view:Node=serde_json::from_value(serde_json::json!({"type":"row","permission":"store.access.manage","children":[{"type":"button","label":"Create","action":"create","permission":"store.access.create"}]})).unwrap();
        view.validate().unwrap();
        let mut store =
            serde_json::json!({"prefs":{"size":30},"access":{"manage":false,"create":true}});
        assert_eq!(view.action_access("create", &store), Some(false));
        merge_store(&mut store, &serde_json::json!({"access":{"manage":true}}));
        assert_eq!(view.action_access("create", &store), Some(true));
        assert_eq!(store["prefs"]["size"], 30);
        merge_store(&mut store, &serde_json::json!({"access":{"create":null}}));
        assert_eq!(view.action_access("create", &store), Some(false));
        assert!(store["access"]["create"].is_null());
        assert!(!permission_allowed(
            &serde_json::json!({"access":{"create":"true"}}),
            Some("store.access.create")
        ));
        assert!(permission_allowed(&Value::Null, None));
        let invalid:Node=serde_json::from_value(serde_json::json!({"type":"text","text":"X","permission":"store.access.create || true"})).unwrap();
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn buffer_round_trip_uses_owner_release() {
        let buffer = Buffer::json(&serde_json::json!({"unicode":"插件"}));
        let value: Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(buffer.ptr, buffer.len) })
                .unwrap();
        assert_eq!(value["unicode"], "插件");
        unsafe { release_buffer(buffer) };
    }
    #[test]
    fn table_accepts_legacy_columns_and_rejects_invalid_widths() {
        let mut node:Node=serde_json::from_value(serde_json::json!({"type":"table","id":"rows","columns":[{"field":"number","label":"#"}],"rows":[],"row_key":"number","on_select":"view"})).unwrap();
        node.validate().unwrap();
        if let Node::Table { columns, .. } = &mut node {
            columns[0].width = Some(-1.0);
        }
        assert!(node.validate().is_err());
    }
}
