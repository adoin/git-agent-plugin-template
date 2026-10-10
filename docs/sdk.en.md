# Plugin interface reference

ABI v1 exchanges UTF-8 JSON. Use export_plugin! from the vendored Rust SDK or include/git_agent_plugin.h. Each allocator releases its own buffers.

## Messages and state

Request carries action, page, context, inputs, state, store, confirmed and event. Context has root, branch, head, remotes and language; HEAD may be unavailable until history loads. Fetch and freeze required remote revisions in the plugin.

Response.ui is a Node tree. state replaces business state, inputs updates form bindings, store_patch uses JSON merge patch, navigate changes registered pages, and badge/repository_tab_count return counts. Sync hooks use state_patch to preserve drafts and confirmations.

confirmation requests a concrete write preview. The host re-invokes the captured action with confirmed=true; handlers must recheck permissions, target and SHA. Refresh unknown outcomes before retrying remote writes.

## Drivers and contributions

basic.v1 renders Page, Row, Column, Form, Text, Input, Select, Button, Navigate, IconText, ActionBar, Viewport, Pagination, Table, Disclosure and Guard. code.v1 renders Code; diff.v1 renders DiffBrowser.

Viewport keeps top and bottom fixed and scrolls content. Table supports fixed/flexible columns, value_labels and semantic value_badges. DiffBrowser takes files with path, patch, additions, deletions and truncated; labels has files/empty/unavailable/truncated. Code preview is capped at 40000 characters and each file diff at 100000 characters.

Layouts have at most 32 levels and 2000 nodes; tables have at most 500 rows. Messages are limited to 8 MiB. The host owns fonts, colors, SVG caching and native rendering.

contributes.pages declares page IDs, localized titles, icons, entry actions, badges, parents and permission. toolbar.inline_pages renders direct entries; toolbar.pages supplies remaining overflow entries. Commands target pages/actions in repository, actions, tools and view menus.

locales maps language codes to JSON dictionaries. Titles accept {"key":"home"}; strings use {{i18n:key}}. Select options and raw data keep wire values; option_labels/value_labels provide translations.

## Scope and services

Any node can declare permission="store.access.increment". Parent and child guards combine and only boolean true grants access. action_permissions checks the authoritative store at dispatch. Recheck real business authority in handlers.

Omitting repository_match enables all repository domains. remote_hosts supports exact and *.example.com matches; host_env can name an additional enterprise-host environment variable. Host global/repository switches apply in addition to these conditions.

Host.call supports cancelled, store.get, store.patch and ai.review. Store snapshots are at most 1 MiB and remain session scoped. AI service use requires the ai capability; the host owns credentials. Declare network and remote.write as appropriate.

repository.remote_sync.started runs asynchronously alongside fetch/pull/push/background fetch. Its event has operation_id, operation and source. It is read-only and cannot navigate or confirm writes. Completed events can refresh from a new local snapshot.

Handlers run off the paint thread, are serialized per library and scoped per repository. Bound child-process time/output and cooperate with cancellation. Do not retain HostApi pointers, borrowed buffers or background work using them beyond a call.

required_tools optionally lists command names such as gh. The manager checks PATH on a worker without executing commands and shows missing tools. Refresh after installing them. One library instance and call gate per plugin ID are retained for the host session; switches stop dispatch, while library replacement/deletion happens after restart.
