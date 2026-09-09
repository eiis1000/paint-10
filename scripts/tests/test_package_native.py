"""Checks of bundle metadata and executable preservation that run on any host."""

import importlib.util
import os
from pathlib import Path
import plistlib
import struct
import tarfile
import tempfile
import unittest
import zipfile


SCRIPT = Path(__file__).resolve().parents[1] / "package-native.py"
SPEC = importlib.util.spec_from_file_location("package_native", SCRIPT)
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class NativePackageTests(unittest.TestCase):
    def test_macos_bundle_and_archive_preserve_launch_metadata_and_executable(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            binary = work / "binary"
            binary.write_bytes(b"executable fixture")
            icon = work / "icon.icns"
            icon.write_bytes(b"icon fixture")
            bundle = package.create_macos_bundle(binary, icon, "0.7.2", work)
            with (bundle / "Contents/Info.plist").open("rb") as file:
                metadata = plistlib.load(file)
            self.assertEqual(metadata["CFBundleDisplayName"], "Paint 10")
            self.assertEqual(metadata["CFBundlePackageType"], "APPL")
            self.assertEqual(metadata["CFBundleVersion"], "0.7.2")
            executable = bundle / "Contents/MacOS" / metadata["CFBundleExecutable"]
            self.assertEqual(executable.read_bytes(), binary.read_bytes())
            icon_copy = bundle / "Contents/Resources" / metadata["CFBundleIconFile"]
            self.assertEqual(icon_copy.read_bytes(), icon.read_bytes())

            output = work / "package.tar.gz"
            package.archive_directory(bundle, output)
            with tarfile.open(output) as archive:
                archived = archive.getmember("Paint 10.app/Contents/MacOS/paint-10")
                if os.name != "nt":
                    self.assertEqual(archived.mode, 0o755)
                self.assertEqual(archive.extractfile(archived).read(), binary.read_bytes())
                self.assertIn("Paint 10.app/Contents/Resources/LICENSE", archive.getnames())
                for notice in ("RaTeX-LICENSE.txt", "KaTeX-fonts-NOTICE.txt", "SIL-OFL-1.1.txt"):
                    self.assertIn(
                        f"Paint 10.app/Contents/Resources/licenses/{notice}", archive.getnames()
                    )

    def test_windows_archive_preserves_binary_and_license(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            directory = work / "paint-10"
            directory.mkdir()
            (directory / "paint-10.exe").write_bytes(b"MZ executable fixture")
            (directory / "LICENSE").write_text("license fixture")
            package.copy_licenses(directory / "licenses")
            output = work / "package.zip"
            package.archive_directory(directory, output)
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(
                    archive.read("paint-10/paint-10.exe"), b"MZ executable fixture"
                )
                self.assertEqual(archive.read("paint-10/LICENSE"), b"license fixture")
                for notice in ("RaTeX-LICENSE.txt", "KaTeX-fonts-NOTICE.txt", "SIL-OFL-1.1.txt"):
                    self.assertEqual(
                        archive.read(f"paint-10/licenses/{notice}"),
                        (package.ROOT / "assets/licenses" / notice).read_bytes(),
                    )

    def test_icon_sources_include_all_windows_sizes_and_full_macos_resolution(self):
        images = package.ico_images(package.ROOT / "assets/paint-10.ico")
        self.assertEqual(
            [(width, height) for width, height, _ in images],
            [(size, size) for size in (256, 128, 64, 48, 32, 24, 16)],
        )
        self.assertEqual(
            package.png_dimensions(package.ROOT / "assets/paint-10-1024.png"),
            (1024, 1024),
        )

    def test_icon_reader_rejects_a_truncated_image(self):
        with tempfile.TemporaryDirectory() as temporary:
            icon = Path(temporary) / "truncated.ico"
            header = struct.pack("<HHH", 0, 1, 1)
            image = struct.pack("<BBBBHHII", 16, 16, 0, 0, 1, 32, 10, 22)
            icon.write_bytes(header + image + b"short")
            with self.assertRaisesRegex(ValueError, "Invalid ICO image"):
                package.ico_images(icon)


if __name__ == "__main__":
    unittest.main()
