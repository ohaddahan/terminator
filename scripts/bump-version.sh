#!/usr/bin/env bash
# Requires Python 3.11+. Updates workspace packages without resolving dependencies.
set -euo pipefail

if (( $# != 0 )); then
    echo "Usage: $0" >&2
    exit 1
fi

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
python3 - "$repo_root" <<'PY'
import pathlib
import re
import sys
import tomllib

root = pathlib.Path(sys.argv[1])
manifest = root / "Cargo.toml"
lockfile = root / "Cargo.lock"
source = manifest.read_text()
workspace = tomllib.loads(source)["workspace"]
old = workspace["package"]["version"]
match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", old)
if not match:
    sys.exit(f"Expected a stable major.minor.patch workspace version, got {old!r}")
new = f"{match[1]}.{int(match[2]) + 1}.0"

names = set()
for member in workspace["members"]:
    for directory in root.glob(member):
        package = tomllib.loads((directory / "Cargo.toml").read_text())["package"]
        if package.get("version") == {"workspace": True}:
            names.add(package["name"])

updated, count = re.subn(
    r'(\[workspace\.package\][\s\S]*?^version\s*=\s*")' + re.escape(old) + r'(")',
    lambda m: m[1] + new + m[2], source, count=1, flags=re.MULTILINE,
)
if count != 1:
    sys.exit("Could not locate workspace version; no files changed")

lock = lockfile.read_text()
seen = set()

def bump_package(match):
    block = match[0]
    package = tomllib.loads(block)["package"][0]
    name = package["name"]
    if name not in names or "source" in package:
        return block
    if package["version"] != old or name in seen:
        sys.exit(f"Unexpected lockfile entry for {name}; no files changed")
    seen.add(name)
    return re.sub(r'^version = "' + re.escape(old) + r'"',
                  f'version = "{new}"', block, count=1, flags=re.MULTILINE)

updated_lock = re.sub(r'\[\[package\]\][\s\S]*?(?=\[\[package\]\]|\Z)', bump_package, lock)
if seen != names:
    sys.exit("Workspace packages missing from Cargo.lock; no files changed")
tomllib.loads(updated)
tomllib.loads(updated_lock)
manifest.write_text(updated)
lockfile.write_text(updated_lock)
print(f"Bumped workspace version: {old} -> {new}")
PY
