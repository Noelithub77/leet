"""Install editor tools privately; run with Leet's verified portable Python."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile


def run(args):
    subprocess.run(args, check=True, stdin=subprocess.DEVNULL,
                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)


def install(root, language):
    metadata = json.loads((root / "tools.json").read_text())
    if language == "python":
        print("Installing basedpyright…", flush=True)
        arch = {"AMD64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}[platform.machine()]
        system = {"Windows": "windows", "Darwin": "macos", "Linux": "linux"}[platform.system()]
        requirements = root / "pinned-wheels.txt"
        requirements.write_text("".join(asset["url"] + " --hash=sha256:" + asset["sha256"] + "\n"
                                       for asset in metadata["wheels"][system + "-" + arch]))
        run([sys.executable, "-m", "pip", "--isolated", "install", "--disable-pip-version-check", "--no-cache-dir",
             "--no-deps", "--require-hashes", "-r", str(requirements)])
        packages = [Path(p) for p in sys.path if p and "site-packages" in p]
        nodes = [node for path in packages for node in path.rglob("node.exe" if os.name == "nt" else "node") if node.is_file()]
        scripts = [script for path in packages for script in path.glob("basedpyright/dist/pyright-langserver.js") if script.is_file()]
        if len(nodes) != 1 or len(scripts) != 1:
            raise RuntimeError("Could not locate basedpyright's bundled Node runtime and language server")
        run([str(nodes[0]), "--version"])
        (root / "python-server.json").write_text(json.dumps({"binary": str(nodes[0]), "script": str(scripts[0])}))
    else:
        if platform.system() == "Linux" and platform.machine() != "x86_64":
            raise RuntimeError("The pinned upstream clangd archive supports Linux x64 only")
        name = {"Linux": "linux", "Darwin": "mac", "Windows": "windows"}[platform.system()]
        asset = metadata["clangd"][name]
        with tempfile.TemporaryDirectory(prefix=".clangd-", dir=root) as directory:
            stage = Path(directory)
            archive = stage / "clangd.zip"
            print("Downloading clangd…", flush=True)
            request = urllib.request.Request(asset["url"], headers={"User-Agent": "leet-tool-setup"})
            for attempt in range(3):
                digest = hashlib.sha256()
                count = 0
                try:
                    with urllib.request.urlopen(request, timeout=60) as response, archive.open("wb") as output:
                        while chunk := response.read(1024 * 1024):
                            count += len(chunk)
                            if count > 512 * 1024 * 1024:
                                raise RuntimeError("Tool archive exceeded its size limit")
                            digest.update(chunk)
                            output.write(chunk)
                    break
                except OSError:
                    if attempt == 2:
                        raise
                    time.sleep(attempt + 1)
            if digest.hexdigest() != asset["sha256"]:
                raise RuntimeError("clangd checksum mismatch")
            with zipfile.ZipFile(archive) as source:
                if sum(member.file_size for member in source.infolist()) > 1024 * 1024 * 1024:
                    raise RuntimeError("Expanded tool archive exceeded its size limit")
                for member in source.infolist():
                    target = (stage / member.filename).resolve()
                    if not target.is_relative_to(stage.resolve()) or member.file_size > 512 * 1024 * 1024:
                        raise RuntimeError("Unsafe tool archive entry")
                    source.extract(member, stage)
                    if os.name != "nt" and member.external_attr >> 16:
                        target.chmod((member.external_attr >> 16) & 0o777)
            binaries = list(stage.glob("clangd_*/bin/clangd.exe" if os.name == "nt" else "clangd_*/bin/clangd"))
            if len(binaries) != 1:
                raise RuntimeError("clangd binary missing from archive")
            binary = binaries[0]
            if os.name != "nt":
                binary.chmod(binary.stat().st_mode | 0o700)
            run([str(binary), "--version"])
            destination = root / "clangd"
            if destination.exists():
                raise RuntimeError("clangd is already installed")
            shutil.move(str(binary.parent.parent), destination)
    print("Tools installed.", flush=True)


if __name__ == "__main__":
    try:
        directory = Path(sys.argv[1]).resolve()
        language = sys.argv[2]
        if language not in {"python", "cpp", "c"}:
            raise ValueError("Unsupported tool setup language")
        install(directory, language)
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
