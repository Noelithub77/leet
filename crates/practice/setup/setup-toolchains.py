"""Verified private toolchain archives; invoked only by the Rust setup operator."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import zipfile


def report(message):
    print(message, flush=True)


def run(args, env=None, label=None):
    report(label or "Checking " + Path(args[0]).name)
    subprocess.run([str(arg) for arg in args], check=True, stdin=subprocess.DEVNULL,
                   env=env, timeout=600,
                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)


def download_once(asset, destination, name):
    report("Downloading " + name)
    request = urllib.request.Request(asset["url"], headers={"User-Agent": "leet-tool-setup"})
    digest = hashlib.sha256()
    count = 0
    last = 0
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("wb") as output:
        total = int(response.headers.get("Content-Length", 0))
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            if count > 512 * 1024 * 1024:
                raise RuntimeError("Tool archive exceeded 512 MiB")
            output.write(chunk)
            digest.update(chunk)
            if time.monotonic() - last >= 0.5 or count == total:
                detail = f"{count / 1048576:.1f} MiB"
                if total:
                    detail = f"{count * 100 // total}% · {count / 1048576:.1f} / {total / 1048576:.1f} MiB"
                report(f"Downloading {name} · {detail}")
                last = time.monotonic()
    report("Verifying " + name)
    if digest.hexdigest() != asset["sha256"]:
        raise RuntimeError(name + " checksum mismatch; retry setup")


def download(asset, destination, name):
    for attempt in range(3):
        try:
            download_once(asset, destination, name)
            return
        except OSError:
            if attempt == 2:
                raise
            report(f"Downloading {name} · retry {attempt + 1}/2")
            time.sleep(attempt + 1)


def extract(archive, stage):
    report("Unpacking " + archive.name)
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as source:
            if sum(member.file_size for member in source.infolist()) > 2 * 1024**3:
                raise RuntimeError("Expanded tool archive exceeded 2 GiB")
            for member in source.infolist():
                target = stage / member.filename
                if not target.resolve().is_relative_to(stage.resolve()):
                    raise RuntimeError("Unsafe tool archive path")
                source.extract(member, stage)
                if os.name != "nt" and not member.is_dir():
                    target.chmod((member.external_attr >> 16) & 0o777 or 0o644)
    else:
        with tarfile.open(archive) as source:
            if sum(member.size for member in source.getmembers()) > 2 * 1024**3:
                raise RuntimeError("Expanded tool archive exceeded 2 GiB")
            source.extractall(stage, filter="data")


def archive_tool(root, name, asset):
    destination = root / name
    marker = destination / ".leet-asset"
    identity = asset["sha256"]
    executable = asset.get("executable")
    if marker.is_file() and marker.read_text() == identity and (not executable or (destination / executable).is_file()):
        return destination
    with tempfile.TemporaryDirectory(prefix=".tool-", dir=root) as temporary:
        stage = Path(temporary)
        archive = stage / ("archive.zip" if asset["url"].endswith((".zip", ".vsix")) else "archive.tar")
        download(asset, archive, name)
        expanded = stage / "expanded"
        expanded.mkdir()
        extract(archive, expanded)
        source = expanded / asset["root"]
        if asset["root"] == "*":
            candidates = [p for p in expanded.iterdir() if p.is_dir()]
            if len(candidates) != 1:
                raise RuntimeError(name + " archive has no unique tool root")
            source = candidates[0]
        if not source.is_dir() or (executable and not (source / executable).is_file()):
            raise RuntimeError(name + " archive is missing its executable")
        if executable and os.name != "nt":
            binary = source / executable
            binary.chmod(binary.stat().st_mode | 0o700)
        # Keep the previous verified tool until extraction has finished.
        previous = root / ("." + name + "-previous")
        if previous.exists():
            shutil.rmtree(previous)
        if destination.exists():
            destination.rename(previous)
        try:
            shutil.move(str(source), destination)
            marker.write_text(identity)
        except BaseException:
            if destination.exists():
                shutil.rmtree(destination)
            if previous.exists():
                previous.rename(destination)
            raise
        if previous.exists():
            shutil.rmtree(previous)
    return destination


def install(root, language):
    catalog = json.loads((root / "toolchains.json").read_text())
    arch = {"AMD64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}[platform.machine()]
    system = {"Windows": "windows", "Darwin": "macos", "Linux": "linux"}[platform.system()]
    packages = catalog["platforms"][system + "-" + arch]
    suffix = ".exe" if os.name == "nt" else ""
    if language == "python":
        wheel = catalog["debugpy"]
        with tempfile.TemporaryDirectory(prefix=".debugpy-", dir=root) as temporary:
            path = Path(temporary) / Path(wheel["url"]).name
            download(wheel, path, "Python debugger")
            report("Installing Python debugger")
            run([sys.executable, "-m", "pip", "--isolated", "install", "--no-deps", "--no-index", str(path)])
        run([sys.executable, "-c", "import debugpy; print(debugpy.__version__)"])
    elif language in {"c", "cpp"}:
        if system == "linux" and arch == "aarch64":
            asset = {**catalog["clangd-arm64"], "root": "clangd/data", "executable": "bin/clangd"}
            archive_tool(root, "clangd", asset)
        compiler = archive_tool(root, "zig", packages["zig"])
        adapter = archive_tool(root, "codelldb", packages["codelldb"])
        run([compiler / ("zig" + suffix), "version"])
        run([adapter / "adapter" / ("codelldb" + suffix), "--help"])
        target = arch + "-" + {"linux": "linux-gnu", "macos": "macos", "windows": "windows-gnu"}[system]
        env = os.environ.copy()
        env["ZIG_GLOBAL_CACHE_DIR"] = str(root / "zig-cache")
        flags = {}
        with tempfile.TemporaryDirectory(prefix=".compiler-", dir=root) as temporary:
            for kind, mode in [("c", "cc"), ("cpp", "c++")]:
                source = Path(temporary) / ("check.c" if kind == "c" else "check.cpp")
                source.write_text("#include <stdio.h>\nint main(void){return 0;}\n" if kind == "c" else "#include <iostream>\nint main(){std::cout << 1;return 0;}\n")
                report("Checking " + mode + " headers")
                result = subprocess.run([str(compiler / ("zig" + suffix)), mode, "-target", target, "-E", "-v", str(source)],
                    env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, timeout=120,
                    creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
                search = result.stderr.split("#include <...> search starts here:", 1)[1].split("End of search list.", 1)[0]
                includes = [line.strip() for line in search.splitlines() if (root / "zig").resolve() in Path(line.strip()).resolve().parents]
                if not includes:
                    raise RuntimeError("Compiler did not report private standard headers")
                flags[kind] = ["--target=" + target, "-nostdinc", "-I", str(root / "include")]
                for include in includes:
                    flags[kind].extend(["-isystem", include])
                run([compiler / ("zig" + suffix), mode, "-target", target, str(source), "-o", Path(temporary) / ("check" + suffix)], env, "Preparing " + mode + " compiler")
        (root / "compiler-flags.json").write_text(json.dumps(flags))
    elif language == "go":
        go = archive_tool(root, "go", packages["go"])
        delve = archive_tool(root, "delve", packages["delve"])
        run([go / "bin" / ("go" + suffix), "version"])
        run([delve / ("dlv" + suffix), "version"])
        env = os.environ.copy()
        env.update(GOROOT=str(go), GOPATH=str(root / "go-work"), GOCACHE=str(root / "go-cache"),
                   GOBIN=str(root / "go-tools/bin"), CGO_ENABLED="0", GOTOOLCHAIN="local", GOENV="off", GOMAXPROCS="2", GOFLAGS="-p=2")
        run([go / "bin" / ("go" + suffix), "install", "golang.org/x/tools/gopls@" + catalog["gopls"]], env, "Installing Go language server")
    elif language == "java":
        jdk = archive_tool(root, "jdk", packages["jdk"])
        archive_tool(root, "jdtls", catalog["jdtls"])
        archive_tool(root, "java-debug", catalog["java-debug"])
        home = jdk / "Contents/Home" if system == "macos" else jdk
        run([home / "bin" / ("java" + suffix), "-version"])
        run([home / "bin" / ("javac" + suffix), "-version"])
    report("Tools installed")


if __name__ == "__main__":
    try:
        if hasattr(os, "sched_getaffinity"):
            os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
        if os.name != "nt":
            os.nice(10)
        install(Path(sys.argv[1]).resolve(), sys.argv[2])
    except Exception as error:
        print(str(error), file=sys.stderr, flush=True)
        sys.exit(1)
