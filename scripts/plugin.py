"""Standalone build, package, initialize and watch commands; no host source checkout required."""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT


def run(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True,
                          creationflags=0x08000000 if os.name == "nt" else 0, **kwargs)


def host_target():
    output = run(["rustc", "-vV"], capture_output=True, text=True).stdout
    return next(line.split(": ", 1)[1] for line in output.splitlines() if line.startswith("host: "))


def manifest():
    return json.loads((PROJECT / "plugin.json").read_text(encoding="utf-8"))


def platform(target):
    if "-windows-" in target:
        return "windows"
    if target.endswith("-apple-darwin"):
        return "macos"
    if "-linux-" in target:
        return "linux"
    raise ValueError("Unsupported plugin target: " + target)


def files(data, library):
    result = {"plugin.json": PROJECT / "plugin.json", data["library"][platform(library[0])]: library[1], "LICENSE": ROOT / "LICENSE"}
    for item in (PROJECT / "assets").rglob("*"):
        if item.is_file():
            result[item.relative_to(PROJECT).as_posix()] = item
    for relative in data.get("locales", {}).values():
        path = (PROJECT / relative).resolve()
        if not path.is_relative_to(PROJECT) or not path.is_file():
            raise ValueError("Dictionary escapes template: " + relative)
        result[relative] = path
    return result


def build(target=None, release=False):
    target = target or host_target()
    data = manifest()
    import tomllib
    cargo = tomllib.loads((PROJECT / "Cargo.toml").read_text(encoding="utf-8"))
    if cargo["package"]["version"] != data["version"]:
        raise ValueError("Cargo and plugin.json versions must match")
    args = ["cargo", "build", "--manifest-path", str(PROJECT / "Cargo.toml"), "--locked", "--lib", "--target", target]
    if release:
        args.append("--release")
    run(args)
    library = ROOT / "target" / target / ("release" if release else "debug") / data["library"][platform(target)]
    if not library.is_file():
        raise ValueError("Library output does not match plugin.json: " + str(library))
    return data, target, library


def package(target):
    data, target, library = build(target, release=True)
    metadata = {"format": 1, "id": data["id"], "version": data["version"],
                "api_version": data["api_version"], "target": target, "min_host_version": "1.4.8"}
    directory = ROOT / "dist"
    directory.mkdir(exist_ok=True)
    output = directory / f"{data['id']}-{data['version']}-{target}.gaplugin"
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("package.json", json.dumps(metadata, indent=2) + "\n")
        for name, path in files(data, (target, library)).items():
            archive.write(path, name)
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + ".sha256").write_text(f"{digest}  {output.name}\n", encoding="utf-8")
    print(output)


def initialize(plugin_id, name, example):
    if example == "minimal":
        if manifest()["id"] not in {"github", "example"}:
            raise ValueError("Select --example github to rename an existing custom plugin without replacing its source")
        backup = ROOT / "target" / "init-backup" / str(time.time_ns())
        for directory in ["src", "assets", "locales"]:
            destination = (ROOT / directory).resolve()
            if not destination.is_relative_to(ROOT) or destination.name != directory:
                raise ValueError("Initialization path escaped the template")
            shutil.copytree(destination, backup / directory)
            shutil.rmtree(destination)
            shutil.copytree(ROOT / "examples" / "minimal" / directory, destination)
        shutil.copyfile(ROOT / "plugin.json", backup / "plugin.json")
        shutil.copyfile(ROOT / "examples/minimal/plugin.json", ROOT / "plugin.json")
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", plugin_id):
        raise ValueError("ID must use lowercase letters, numbers and hyphens")
    data = manifest()
    data["id"] = plugin_id
    data["name"] = name or plugin_id
    library = "git_agent_plugin_" + plugin_id.replace("-", "_")
    data["library"] = {"windows": library + ".dll", "linux": "lib" + library + ".so", "macos": "lib" + library + ".dylib"}
    (PROJECT / "plugin.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    cargo = re.sub(r'(?m)^name = "[^"]+"', 'name = "git-agent-plugin-' + plugin_id + '"', cargo, count=1)
    cargo = re.sub(r'(\[lib\]\s*\n)name = "[^"]+"', r'\1name = "' + library + '"', cargo)
    (ROOT / "Cargo.toml").write_text(cargo, encoding="utf-8")
    run(["cargo", "generate-lockfile"])
    print("Initialized " + plugin_id)


def stage():
    data, target, library = build()
    directory = ROOT / "target" / "dev-plugins" / data["id"]
    directory.mkdir(parents=True, exist_ok=True)
    for name, source in files(data, (target, library)).items():
        destination = directory / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    print("Staged: " + str(directory))
    return directory.parent


def fingerprint():
    return tuple((str(file), file.stat().st_mtime_ns) for file in sorted(ROOT.rglob("*"))
                 if file.is_file() and not any(part in {"target", "dist", ".git", "__pycache__"} for part in file.relative_to(ROOT).parts))


def develop(executable, smoke):
    process = None
    def stop():
        nonlocal process
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        process = None
    def refresh():
        nonlocal process
        stop()
        directory = stage()
        if executable:
            env = os.environ.copy()
            env["GIT_AGENT_PLUGIN_DIR"] = str(directory)
            kwargs = {}
            if smoke and os.name == "nt":
                startup = subprocess.STARTUPINFO()
                startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
                startup.wShowWindow = 0
                kwargs["startupinfo"] = startup
            process = subprocess.Popen([str(Path(executable).resolve())], cwd=ROOT, env=env, **kwargs)
    try:
        refresh()
        if smoke:
            time.sleep(smoke)
            if process and process.poll() is not None:
                raise RuntimeError("Host exited during smoke check")
            return
        previous = fingerprint()
        print("Watching plugin sources; Ctrl+C stops only this script's host process.")
        while True:
            time.sleep(0.5)
            current = fingerprint()
            if current != previous:
                time.sleep(0.3)
                try:
                    refresh()
                except subprocess.CalledProcessError:
                    print("Build failed; correct the source to retry.")
                previous = fingerprint()
    except KeyboardInterrupt:
        pass
    finally:
        stop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    pack = sub.add_parser("package")
    pack.add_argument("--target")
    pack.add_argument("--example", choices=["github", "minimal"], default="github")
    init = sub.add_parser("init")
    init.add_argument("id")
    init.add_argument("--name")
    init.add_argument("--example", choices=["github", "minimal"], default="minimal")
    dev = sub.add_parser("dev")
    dev.add_argument("--host")
    dev.add_argument("--example", choices=["github", "minimal"], default="github")
    dev.add_argument("--smoke", type=int, default=0)
    args = parser.parse_args()
    if args.command != "init" and args.example == "minimal":
        PROJECT = ROOT / "examples/minimal"
    if args.command == "package":
        package(args.target)
    elif args.command == "init":
        initialize(args.id, args.name, args.example)
    else:
        develop(args.host, args.smoke)
