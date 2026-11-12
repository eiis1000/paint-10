"""Exercise the CI tool installer without network access or global installs."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


INSTALLER = Path(__file__).resolve().parents[1] / "install-wasm-bindgen.sh"


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "Linux CI installer")
class WasmBindgenInstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"],
                        CARGO_HOME=str(self.root / "cargo"))

    def command(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/sh\n" + body)
        path.chmod(0o755)

    def run_installer(self):
        return subprocess.run(["bash", str(INSTALLER)], env=self.env,
                              capture_output=True, text=True)

    def test_matching_cached_binary_does_not_download(self):
        self.command("wasm-bindgen", 'echo "wasm-bindgen 0.2.127"\n')
        self.command("curl", 'echo "unexpected download" >&2\nexit 99\n')
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("unexpected download", result.stderr)
        self.assertFalse((self.root / "cargo").exists())

    def test_bad_checksum_does_not_replace_existing_binary(self):
        self.command("wasm-bindgen", 'echo "wasm-bindgen old-version"\n')
        self.command("curl", '''while [ "$1" != "--output" ]; do shift; done
shift
printf 'invalid archive' > "$1"
''')
        installed = self.root / "cargo/bin/wasm-bindgen"
        installed.parent.mkdir(parents=True)
        installed.write_bytes(b"existing binary")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("FAILED", result.stdout)
        self.assertEqual(installed.read_bytes(), b"existing binary")


if __name__ == "__main__":
    unittest.main()
