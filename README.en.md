# Git Agent plugin template

Develop, test and package a native Git Agent plugin in this independent repository. The example has two pages, Chinese and English dictionaries, an asynchronous sync hook and store-based permissions. The host renders the UI. SDK 0.1.0 and ABI v1 are pinned in vendor; the host source checkout is not a build dependency.

[中文指南](README.md) · [SDK reference](docs/sdk.en.md)

## Requirements

Use Rust stable with edition 2024 support and Python 3.10 or later. Windows wrappers use PowerShell 7; MSVC builds require C++ build tools. Use a Git Agent build containing API v1 and plugin management. This template was verified against the current 1.4.8 development build.

## Initialize

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/init.ps1 -Id my-plugin -Name "My plugin"
```

The script changes the Cargo package, library names, manifest identity and lockfile. Implement actions in src/lib.rs; declare contributions in plugin.json; edit locales and assets. IDs must be unique. When updating, change version in Cargo.toml and plugin.json together and run cargo generate-lockfile.

## Test and develop

```powershell
cargo test --workspace --locked
cargo fmt --all --check
pwsh.exe -NoLogo -NoProfile -File scripts/dev.ps1 -HostExecutable D:/workspace/git-Agent/target/debug/git-agent.exe
```

The script builds, stages into target/dev-plugins and launches the host with GIT_AGENT_PLUGIN_DIR. It watches source and resources, stopping only its own host process before rebuilding. Ctrl+C stops that process too. Without HostExecutable it only builds, stages and watches. Add -SmokeTestSeconds 5 for a bounded smoke test. On Linux/macOS use python scripts/plugin.py dev --host /path/to/git-agent.

Managed plugins still load; a development plugin takes precedence over an installed plugin with the same ID. Global and repository switches remain effective.

## Package and install

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/package.ps1
```

On Linux/macOS use python scripts/plugin.py package. Packages appear at dist/ID-VERSION-TARGET.gaplugin. Pass -Target or --target for another installed Rust target and linker.

Import the package in Options → Plugins → Install / update package. New plugins load after installation; newer versions of an installed plugin take effect after restart. Global switches control all repositories. Repository Settings → Plugins supports inheritance and repository overrides; an override never bypasses a global disable or domain mismatch.

Uninstall stops new calls and lets current tasks finish cooperatively; files are removed on restart. Host preferences remain. Development source directories are maintained by the developer and are not removed by managed uninstall.

A package is ZIP with plugin.json, package.json, the target native library, dictionaries and assets at its root. Installation checks API, target, version, resources and paths without loading native code. Enabling checks the actual ABI entry. Native plugins run inside the host process; host-service declarations and store permissions are not a system sandbox.

## Example and SDK

The example matches all repositories because it omits repository_match. Its counter button is guarded by store.access.increment at UI and dispatch level, with a final handler check. Preferences simulate permission updates. The remote-sync-started hook updates a participation count without contacting external services.

Inputs, state and permission store are isolated per plugin and repository within the app session. Example names, counters and access flags are not durable; installation records and global/repository switches are durable.

The vendored SDK snapshot is tracked in vendor/sdk-version.json. Upgrade it deliberately, update the checksum and dependency version, then retest. CI checks formatting and tests and uploads packages for the actual Windows/Linux/macOS runner target. Licensing and attribution are in LICENSE.
