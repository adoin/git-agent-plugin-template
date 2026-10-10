"""Stage and publish the configured root plugin, independently of its ID."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent
TARGETS = (
    "x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin", "x86_64-apple-darwin",
)


def metadata(root, tag=None):
    plugin = json.loads((root / "plugin.json").read_text(encoding="utf-8"))
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", plugin["id"]):
        raise ValueError("Invalid plugin ID")
    if cargo["package"]["version"] != plugin["version"]:
        raise ValueError("Cargo and plugin manifest versions must match")
    if tag is not None and tag != "v" + plugin["version"]:
        raise ValueError("Release tag and plugin version must match")
    return plugin


def stage(root):
    plugin = metadata(root)
    dist = root / "dist"
    packages = sorted(dist.glob(f"{plugin['id']}-{plugin['version']}-*.gaplugin"))
    if not packages:
        raise ValueError("No root plugin packages were generated")
    for package in packages:
        if not package.with_name(package.name + ".sha256").is_file():
            raise ValueError("Missing package checksum: " + package.name)
    destination = dist / "primary"
    destination.mkdir(exist_ok=True)
    for package in packages:
        for source in (package, package.with_name(package.name + ".sha256")):
            target = destination / source.name
            if target.exists():
                raise ValueError("Release staging file already exists: " + target.name)
            source.rename(target)


def release_command(root, tag):
    plugin = metadata(root, tag)
    directory = root / "release-assets/primary"
    files = []
    for target in TARGETS:
        package = directory / f"{plugin['id']}-{plugin['version']}-{target}.gaplugin"
        for path in (package, package.with_name(package.name + ".sha256")):
            if not path.is_file():
                raise ValueError("Missing release artifact: " + path.name)
            files.append(str(path))
    command = ["gh", "release", "create", tag, *files, "--verify-tag", "--title",
               f"{plugin['name']} plugin {tag}", "--notes-file", str(root / "docs/release-notes.md")]
    if "-" in plugin["version"].split("+", 1)[0]:
        command.extend(["--prerelease", "--latest=false"])
    return command


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["stage", "publish"])
    parser.add_argument("tag", nargs="?")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.mode == "stage":
        stage(ROOT)
    else:
        if not args.tag:
            parser.error("publish requires a release tag")
        command = release_command(ROOT, args.tag)
        if args.dry_run:
            print(json.dumps(command))
        else:
            subprocess.run(command, check=True)
