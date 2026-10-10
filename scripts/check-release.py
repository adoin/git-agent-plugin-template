"""Validate plugin release tags before uploading immutable plugin artifacts."""
import sys
from pathlib import Path
from release_plugin import metadata

root = Path(__file__).resolve().parent.parent
tag = sys.argv[1]
metadata(root, tag)
print("Validated " + tag)
