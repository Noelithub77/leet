"""Offline archive-integrity and retry regressions for the private tool installer."""
import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("installer", Path(__file__).resolve().parents[1] / "crates/practice/setup/setup-toolchains.py")
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class Response(io.BytesIO):
    def __init__(self, payload):
        super().__init__(payload)
        self.headers = {"Content-Length": str(len(payload))}


class ToolInstallTests(unittest.TestCase):
    def test_checksum_mismatch_preserves_previous_tool(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            previous = root / "compiler"
            previous.mkdir()
            (previous / "working").write_text("keep")
            asset = {"url": "https://fixture.invalid/compiler.zip", "sha256": "0" * 64, "root": "tool"}
            with patch.object(installer.urllib.request, "urlopen", return_value=Response(b"corrupt")):
                with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                    installer.archive_tool(root, "compiler", asset)
            self.assertEqual((previous / "working").read_text(), "keep")
            self.assertFalse((root / ".compiler-previous").exists())

    def test_zip_traversal_cannot_write_outside_staging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "bad.zip"
            with zipfile.ZipFile(archive, "w") as output:
                output.writestr("../escaped", "bad")
            stage = root / "stage"
            stage.mkdir()
            with self.assertRaisesRegex(RuntimeError, "Unsafe tool archive path"):
                installer.extract(archive, stage)
            self.assertFalse((root / "escaped").exists())

    def test_verified_tool_is_installed_once_and_can_retry(self):
        payload = io.BytesIO()
        with zipfile.ZipFile(payload, "w") as output:
            output.writestr("tool/bin/compiler", "fixture")
        payload = payload.getvalue()
        asset = {"url": "https://fixture.invalid/compiler.zip", "sha256": hashlib.sha256(payload).hexdigest(), "root": "tool", "executable": "bin/compiler"}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(installer.urllib.request, "urlopen", return_value=Response(payload)) as fetch:
                installed = installer.archive_tool(root, "compiler", asset)
                self.assertEqual((installed / "bin/compiler").read_text(), "fixture")
                installer.archive_tool(root, "compiler", asset)
                self.assertEqual(fetch.call_count, 1)
            (installed / "bin/compiler").unlink()
            with patch.object(installer.urllib.request, "urlopen", return_value=Response(payload)):
                installer.archive_tool(root, "compiler", asset)
            self.assertTrue((installed / "bin/compiler").is_file())


if __name__ == "__main__":
    unittest.main()
