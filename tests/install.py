"""Exercise the real installer with isolated homes and local release downloads."""
import hashlib
import io
import os
from pathlib import Path
import signal
import subprocess
import tarfile
import tempfile
import unittest

INSTALLER = Path(__file__).resolve().parents[1] / "site/public/install.sh"


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.env = dict(os.environ, HOME=str(self.home), PATH=f"{self.tools}:{os.environ['PATH']}",
                        DISPLAY=":99", FIXTURES=str(self.root), TEST_OS="Linux", TEST_ARCH="x86_64")
        self.tool("uname", 'case "$1" in -s) echo "$TEST_OS";; -m) echo "$TEST_ARCH";; esac')
        self.tool("curl", '''output=
for arg in "$@"; do
  if [ "${next:-}" = yes ]; then output=$arg; next=; fi
  if [ "$arg" = -o ]; then next=yes; fi
  case "$arg" in https://*) url=$arg;; esac
done
if [ "$output" = /dev/null ]; then
  printf 'https://github.com/Noelithub77/leet/releases/tag/v0.1.0'
else
  cp "$FIXTURES/${url##*/}" "$output"
fi''')
        self.tool("ldd", "echo 'libc.so.6 => /lib/libc.so.6'")
        self.tool("nohup", 'echo $$ > "$HOME/launch-pid"; exec /bin/sleep 30')
        self.tool("sleep", "/bin/sleep 0.1")
        self.tool("sw_vers", "echo 13.0")
        self.tool("sysctl", "echo 0")
        self.tool("open", 'printf "%s" "$1" > "$HOME/opened"')
        self.home.joinpath("credentials-preserved").write_text("untouched")

    def tearDown(self):
        pid = self.home / "launch-pid"
        if pid.exists():
            try:
                os.kill(int(pid.read_text()), signal.SIGTERM)
            except ProcessLookupError:
                pass
        self.temp.cleanup()

    def tool(self, name, body):
        path = self.tools / name
        path.write_text(f"#!/bin/sh\nset -eu\n{body}\n")
        path.chmod(0o755)

    def release(self, mac=False):
        asset = self.root / ("leet-macos-aarch64.tar.gz" if mac else "leet-linux-x86_64.tar.gz")
        with tarfile.open(asset, "w:gz") as archive:
            entries = {"leet.app/Contents/MacOS/leet" if mac else "leet": b'#!/bin/sh\necho "leet 0.1.0"\n'}
            if not mac:
                entries["leet.svg"] = b"<svg/>"
            for name, content in entries.items():
                info = tarfile.TarInfo(name)
                info.size = len(content)
                info.mode = 0o755 if name.endswith("leet") else 0o644
                archive.addfile(info, io.BytesIO(content))
        self.root.joinpath("SHA256SUMS").write_text(f"{hashlib.sha256(asset.read_bytes()).hexdigest()}  {asset.name}\n")
        return asset

    def install(self):
        return subprocess.run(["sh", str(INSTALLER)], env=self.env, text=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)

    def test_linux_installs_launches_and_preserves_files(self):
        self.release()
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        command = self.home / ".local/bin/leet"
        self.assertTrue(command.is_symlink())
        self.assertEqual(command.resolve(), self.home.joinpath(".local/bin/1337").resolve())
        self.assertTrue(self.home.joinpath(".local/share/applications/leet.desktop").is_file())
        self.assertTrue(self.home.joinpath("launch-pid").exists())
        self.assertEqual(self.home.joinpath("credentials-preserved").read_text(), "untouched")

    def test_macos_installs_app_and_opens_it(self):
        self.env.update(TEST_OS="Darwin", TEST_ARCH="arm64")
        self.release(mac=True)
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        app = self.home / "Applications/leet.app"
        self.assertTrue(app.is_symlink())
        self.assertEqual(self.home.joinpath("opened").read_text(), str(app))
        self.assertTrue(self.home.joinpath(".local/bin/leet").resolve().is_file())

    def test_checksum_failure_never_installs_or_launches(self):
        asset = self.release()
        asset.write_bytes(asset.read_bytes() + b"tampered")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Checksum verification failed", result.stderr)
        self.assertFalse(self.home.joinpath(".local/bin/leet").exists())
        self.assertFalse(self.home.joinpath("launch-pid").exists())

    def test_existing_executable_is_preserved(self):
        self.release()
        command = self.home / ".local/bin/leet"
        command.parent.mkdir(parents=True)
        command.write_text("personal executable")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Refusing to replace", result.stderr)
        self.assertEqual(command.read_text(), "personal executable")
        self.assertFalse(self.home.joinpath("launch-pid").exists())

    def test_unsupported_architecture_fails_before_downloading(self):
        self.env["TEST_ARCH"] = "riscv64"
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unsupported architecture", result.stderr)


if __name__ == "__main__":
    unittest.main()
