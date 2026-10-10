# Git Agent GitHub plugin and development template

The default project is the cross-platform gh-based GitHub plugin. Its source, version and releases live here. examples/minimal is a minimal starter; vendor pins SDK 0.1.0 and ABI v1. The host owns rendering, hooks and installation and no longer compiles GitHub business code.

[中文](README.md) · [SDK reference](docs/sdk.en.md) · [Release guide](docs/releasing.md)

The plugin supports PR lists and file diffs, creation/management, AI review drafts, reviews, merge/squash/rebase, issues, workflows, releases and repository information. It uses store-based permissions with fresh write checks and SHA pinning. Install gh and authenticate with gh auth login. Remote-domain matching supports github.com and GH_HOST. Sync hooks run alongside host remote operations.

## Develop

Use Rust stable with edition 2024, Python 3.11+, and PowerShell 7 for Windows wrappers. MSVC requires C++ build tools.

```powershell
cargo test --workspace --locked
cargo fmt --all --check
pwsh.exe -NoLogo -NoProfile -File scripts/dev.ps1 -HostExecutable D:/workspace/git-Agent/target/debug/git-agent.exe
```

The default is GitHub. Select -Example minimal or --example minimal to develop/package the starter. The script restarts only its own host process. Host-side dev.ps1 discovers this sibling repository or accepts -ExternalPluginProjects.

## Start another plugin

```powershell
pwsh.exe -NoLogo -NoProfile -File scripts/init.ps1 -Id my-plugin -Name "My plugin" -Example minimal
```

Minimal mode replaces root source/resources and preserves originals in target/init-backup. Select -Example github to retain GitHub functionality while renaming it. Minimal mode refuses to overwrite an already-customized plugin identity. Edit src, plugin.json, locales and assets. Keep Cargo and manifest versions identical and update Cargo.lock.

## Package and release

Run scripts/package.ps1 or python scripts/plugin.py package. dist contains ID-VERSION-TARGET.gaplugin and SHA256 sidecars. CI builds Windows/Linux x86_64 and macOS ARM64/x86_64. The minimal example is also built separately as a regression fixture.

Tag vVERSION after tests and matching version checks. CI reads the ID and name from plugin.json and publishes only the root plugin's packages, including after initializing your own plugin. Versions with prerelease suffixes such as alpha, beta or rc are marked as prereleases. Import packages in Options → Plugins; updates activate after restart.

The host pins repository/tag/version/commit in bundled-plugins.lock.json and stages verified release packages into installers. Release the plugin before the host version that pins it; their versions are independent. GitHub plugin 0.1.0 is published and bundled with Git Agent 2.0.0-beta.3.

## Share your plugin

Publish packages and installation instructions in your own repository, then open a PR against git-Agent's main branch adding a name and repository URL to the Plugin list table in README.md: `| My plugin | https://github.com/you/my-plugin |`. No plugin source, binaries or website edits are needed. The maintainer generates and deploys the Chinese README and website lists after merging. Listing does not automatically bundle a plugin. See the [plugin catalog and sharing guide](https://github.com/adoin/git-Agent#plugins).

ABI v1, driver, permission and packaging details are in docs/sdk.en.md. Native plugins have user-level permissions; host-service/store rules are not a native sandbox. Licensing is in LICENSE.
