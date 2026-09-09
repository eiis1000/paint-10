#!/usr/bin/env python3
"""Package and check the native release on Linux, Windows, or macOS.

Requires Python 3.12 or newer. macOS uses its supplied sips, iconutil and plutil
tools. The startup check exercises the file-dialog protocol without opening UI.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import os
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
APP_NAME = "Paint 10"
BUNDLE_ID = "org.paint10.Paint10"


def png_dimensions(path: Path) -> tuple[int, int]:
    header = path.read_bytes()[:24]
    if header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        raise ValueError(f"Not a PNG: {path}")
    return struct.unpack(">II", header[16:24])


def create_macos_icon(source: Path, destination: Path, work: Path) -> None:
    if png_dimensions(source) != (1024, 1024):
        raise ValueError("The macOS source icon must be 1024 by 1024 pixels")
    iconset = work / "paint-10.iconset"
    iconset.mkdir()
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            suffix = "@2x" if scale == 2 else ""
            image = iconset / f"icon_{size}x{size}{suffix}.png"
            pixels = str(size * scale)
            subprocess.run(
                ["sips", "-z", pixels, pixels, str(source), "--out", str(image)],
                check=True,
                stdout=subprocess.DEVNULL,
            )
    subprocess.run(
        ["iconutil", "-c", "icns", "-o", str(destination), str(iconset)],
        check=True,
    )
    # Check the deployed ICNS, rather than only the images fed to iconutil.
    decoded = work / "decoded.iconset"
    subprocess.run(
        ["iconutil", "-c", "iconset", "-o", str(decoded), str(destination)],
        check=True,
    )
    for original in iconset.iterdir():
        if png_dimensions(decoded / original.name) != png_dimensions(original):
            raise ValueError(f"ICNS lost the resolution of {original.name}")


def create_macos_bundle(binary: Path, icon: Path, version: str, work: Path) -> Path:
    bundle = work / f"{APP_NAME}.app"
    contents = bundle / "Contents"
    executable = contents / "MacOS" / "paint-10"
    resources = contents / "Resources"
    executable.parent.mkdir(parents=True)
    resources.mkdir()
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    shutil.copy2(icon, resources / "paint-10.icns")
    shutil.copy2(ROOT / "LICENSE", resources / "LICENSE")
    metadata = {
        "CFBundleName": APP_NAME,
        "CFBundleDisplayName": APP_NAME,
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleExecutable": executable.name,
        "CFBundleIconFile": "paint-10.icns",
        "CFBundlePackageType": "APPL",
        "CFBundleVersion": version,
        "CFBundleShortVersionString": version,
        "LSApplicationCategoryType": "public.app-category.graphics-design",
        "NSHighResolutionCapable": True,
    }
    with (contents / "Info.plist").open("wb") as file:
        plistlib.dump(metadata, file, sort_keys=True)
    return bundle


def ico_images(path: Path) -> list[tuple[int, int, bytes]]:
    data = path.read_bytes()
    reserved, kind, count = struct.unpack_from("<HHH", data)
    if (reserved, kind) != (0, 1) or not 1 <= count <= 256:
        raise ValueError(f"Invalid ICO directory: {path}")
    images = []
    for index in range(count):
        width, height, _, _, _, _, size, offset = struct.unpack_from(
            "<BBBBHHII", data, 6 + index * 16
        )
        payload = data[offset : offset + size]
        if len(payload) != size or offset < 6 + count * 16 or not payload:
            raise ValueError(f"Invalid ICO image {index}: {path}")
        images.append((width or 256, height or 256, payload))
    return images


def check_windows_icon(binary: Path, icon: Path) -> None:
    """Compare every PE icon resource with its exact source ICO image."""
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    pointer = ctypes.c_void_p
    kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, pointer, ctypes.c_uint32]
    kernel.LoadLibraryExW.restype = pointer
    kernel.FindResourceW.argtypes = [pointer, pointer, pointer]
    kernel.FindResourceW.restype = pointer
    kernel.SizeofResource.argtypes = [pointer, pointer]
    kernel.SizeofResource.restype = ctypes.c_uint32
    kernel.LoadResource.argtypes = [pointer, pointer]
    kernel.LoadResource.restype = pointer
    kernel.LockResource.argtypes = [pointer]
    kernel.LockResource.restype = pointer
    kernel.FreeLibrary.argtypes = [pointer]
    kernel.FreeLibrary.restype = ctypes.c_int

    # Load only resource data. This does not execute the application's code.
    module = kernel.LoadLibraryExW(str(binary), None, 0x00000002 | 0x00000020)
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())

    def resource(identifier: int, kind: int) -> bytes:
        found = kernel.FindResourceW(module, identifier, kind)
        if not found:
            raise ctypes.WinError(ctypes.get_last_error())
        size = kernel.SizeofResource(module, found)
        loaded = kernel.LoadResource(module, found)
        data = kernel.LockResource(loaded)
        if not size or not data:
            raise ctypes.WinError(ctypes.get_last_error())
        return ctypes.string_at(data, size)

    try:
        group = resource(1, 14)  # RT_GROUP_ICON, from assets/paint-10.rc.
        images = ico_images(icon)
        if struct.unpack_from("<HHH", group) != (0, 1, len(images)):
            raise ValueError("The executable's icon group differs from the ICO")
        for index, (width, height, payload) in enumerate(images):
            entry = struct.unpack_from("<BBBBHHIH", group, 6 + index * 14)
            dimensions = (entry[0] or 256, entry[1] or 256, entry[6])
            if dimensions != (width, height, len(payload)) or resource(entry[7], 3) != payload:
                raise ValueError(f"The executable changed or omitted icon image {index}")
    finally:
        kernel.FreeLibrary(module)


def archive_directory(directory: Path, destination: Path) -> None:
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
            for file in sorted(directory.rglob("*")):
                if file.is_file():
                    archive.write(file, file.relative_to(directory.parent))
    else:
        with tarfile.open(destination, "w:gz") as archive:
            archive.add(directory, arcname=directory.name)


def check_startup(binary: Path) -> None:
    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in {"DISPLAY", "WAYLAND_DISPLAY"}
    }
    result = subprocess.run(
        [str(binary), "--paint-10-save-dialog"],
        input=b"{}",
        capture_output=True,
        env=environment,
        timeout=30,
        check=True,
    )
    expected = {"Err": "missing field `initial_format` at line 1 column 2"}
    if json.loads(result.stdout) != expected:
        raise ValueError("The packaged executable failed its startup protocol check")


def main() -> None:
    platform = {"linux": "linux", "darwin": "macos", "win32": "windows"}[sys.platform]
    suffix = ".exe" if platform == "windows" else ""
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=target / f"release/paint-10{suffix}")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    with (ROOT / "Cargo.toml").open("rb") as file:
        version = tomllib.load(file)["package"]["version"].split("-", 1)[0].split("+", 1)[0]
    target.mkdir(parents=True, exist_ok=True)
    extension = "zip" if platform == "windows" else "tar.gz"
    output = target / f"paint-10-{platform}.{extension}"
    with tempfile.TemporaryDirectory(prefix="native-package-", dir=target) as temporary:
        work = Path(temporary)
        if platform == "macos":
            icon = work / "paint-10.icns"
            create_macos_icon(ROOT / "assets/paint-10-1024.png", icon, work)
            directory = create_macos_bundle(binary, icon, version, work)
            subprocess.run(["plutil", "-lint", str(directory / "Contents/Info.plist")], check=True)
            relative_binary = Path("Contents/MacOS/paint-10")
        else:
            directory = work / "paint-10"
            relative_binary = Path(f"paint-10{suffix}")
            directory.mkdir()
            shutil.copy2(binary, directory / relative_binary)
            shutil.copy2(ROOT / "LICENSE", directory / "LICENSE")
            if platform == "windows":
                check_windows_icon(directory / relative_binary, ROOT / "assets/paint-10.ico")
            else:
                (directory / relative_binary).chmod(0o755)
        archive_directory(directory, output)
        extracted = work / "extracted"
        if platform == "windows":
            with zipfile.ZipFile(output) as archive:
                archive.extractall(extracted)
        else:
            with tarfile.open(output) as archive:
                archive.extractall(extracted, filter="data")
        deployed_binary = extracted / directory.name / relative_binary
        if deployed_binary.read_bytes() != binary.read_bytes():
            raise ValueError("The archive did not preserve the release executable")
        check_startup(deployed_binary)
    print(f"Packaged and checked {output}")


if __name__ == "__main__":
    main()
