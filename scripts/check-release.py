"""Validate plugin release tags before uploading immutable plugin artifacts."""
import json
import sys
import tomllib
from pathlib import Path

root = Path(__file__).resolve().parent.parent
plugin = json.loads((root / "plugin.json").read_text(encoding="utf-8"))
cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
tag = sys.argv[1]
if plugin["id"] != "github" or tag != "v" + plugin["version"] or cargo["package"]["version"] != plugin["version"]:
    raise SystemExit("Release tag, Cargo version and GitHub manifest version must match")
print("Validated " + tag)
