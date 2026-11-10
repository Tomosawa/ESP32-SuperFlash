"""Prepare pinned release tools in tools/<os>-<arch>."""

import argparse
import concurrent.futures
import hashlib
import io
import importlib.metadata
import platform
import shutil
import subprocess
import sys
import sysconfig
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / ".cache"
HOST_OS = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}.get(platform.system())
HOST_ARCH = {"arm64": "aarch64", "aarch64": "aarch64", "amd64": "x86_64", "x86_64": "x86_64"}.get(
    platform.machine().lower()
)
HOST = f"{HOST_OS}-{HOST_ARCH}"
TOOLS = ROOT / "tools"
SHARED = TOOLS / "shared"
ESPTOOL = "v5.6.0"
IDF = "v6.1"
FONT = "2894aab31764f10f29c421bdfd2340d3b382d384"
LITTLEFS = [("4.1.0", "42acb97", "mklittlefs"), ("4.0.2", "db0513a", "mklittlefs-legacy")]
SPIFFS = "0.2.3"
PYTHON_TAG = "20261026"
PYTHON_VERSION = "3.12.16"
# Architecture names and archive checksums published on the upstream release pages.
ESPTOOL_ASSETS = {
    "macos-aarch64": ("arm64", "08a58829a3f14bb08f2e39f4cad48dcb14b7000384268e7f06689836332d6e25"),
    "macos-x86_64": ("amd64", "23921776a7df75f2e6f3ca10a604d74db4661e1b8a0ebb89bdddcd2d26f37055"),
    "windows-x86_64": ("amd64", "d55a96fbeb97fd4cb05bd93f201e5c8f6518b7bfcde2fc6966fd0190fd891629"),
    "linux-x86_64": ("amd64", "1590723cfa9675983f49d0bf2895b8507f8d2879082e51bb7ef02a8bca88d354"),
    "linux-aarch64": ("aarch64", "7723120429394b4d6d974d694e5cf378e00259381a7f8856c2c88492041b9f59"),
}
PYTHON_ASSETS = {
    "macos-aarch64": ("aarch64-apple-darwin", "353d5f1b4fd2715e7053f12f068435d5bbad16b91e057006b5d01e0925709a7e"),
    "macos-x86_64": ("x86_64-apple-darwin", "37c38fda76fec1fbf310adcbafb7f0d96af13b5511bf8ea954c3aa8eec185734"),
    "windows-x86_64": ("x86_64-pc-windows-msvc", "1f2ab53880fc67ea323398c6388e05af28addc99a8e389209f08a51fdff4b149"),
    "linux-x86_64": ("x86_64-unknown-linux-gnu", "ce4f6d0a6cfdb3bed6e44c2f9affd85943c76a42e25ad3a259002f31d6fb8a20"),
    "linux-aarch64": ("aarch64-unknown-linux-gnu", "bb4b6392670c5d3f93876996fcaa19858a4a413a0c7720144c18b0d6f1f0db2d"),
}
SPIFFS_ASSETS = {
    "macos-x86_64": ("osx.tar.gz", "9f43fc74a858cf564966b5035322c3e5e61c31a647c5a1d71b388ed6efc48423"),
    "windows-x86_64": ("win32.zip", "b647f2c2efe6949819c85ea9404271b55c7c9c25bcb98d3b98a1d0ba771adf56"),
    "linux-x86_64": ("linux64.tar.gz", "5e1a4ff41385e842f389f6b5254102a547e566a06b49babeffa93ef37115cb5d"),
}
IDF_BASE = f"https://raw.githubusercontent.com/espressif/esp-idf/{IDF}/"
GOOGLE_BASE = f"https://raw.githubusercontent.com/google/fonts/{FONT}/ofl/"
FALLBACK_FONTS = {"SC": None, "TC": 3, "JP": 4, "KR": 10}
CATALOGUES = [ROOT / "locales/ui.tsv", ROOT / "locales/diagnostics.tsv"]
IDF_FILES = [
    "components/spiffs/spiffsgen.py",
    "components/fatfs/fatfsgen.py",
    "components/fatfs/wl_fatfsgen.py",
    *[
        f"components/fatfs/fatfs_utils/{name}.py"
        for name in (
            "__init__", "boot_sector", "cluster", "entry", "exceptions", "fat",
            "fatfs_state", "fs_object", "long_filename_utils", "utils",
        )
    ],
]


def fetch(url, checksum=None):
    key = hashlib.sha256(url.encode()).hexdigest()[:16]
    file = CACHE / "downloads" / key
    file.parent.mkdir(parents=True, exist_ok=True)
    if file.is_file():
        data = file.read_bytes()
        if checksum is None or hashlib.sha256(data).hexdigest() == checksum:
            return data
    print(f"Downloading {url}", flush=True)
    request = urllib.request.Request(url, headers={"User-Agent": "ESP32-SuperFlash-build"})
    with urllib.request.urlopen(request, timeout=120) as response:
        data = response.read()
    if checksum is not None and hashlib.sha256(data).hexdigest() != checksum:
        raise RuntimeError(f"SHA-256 mismatch for {url}")
    with tempfile.NamedTemporaryFile(dir=file.parent, delete=False) as out:
        out.write(data)
    Path(out.name).replace(file)
    return data

def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def archive_files(url, names, checksum=None):
    data = io.BytesIO(fetch(url, checksum))
    if url.endswith(".zip"):
        with zipfile.ZipFile(data) as archive:
            return {
                Path(name).name: archive.read(name)
                for name in archive.namelist() if Path(name).name in names
            }
    with tarfile.open(fileobj=data, mode="r:gz") as archive:
        return {
            Path(member.name).name: archive.extractfile(member).read()
            for member in archive if member.isfile() and Path(member.name).name in names
        }


def executable(path, data):
    save(path, data)
    path.chmod(0o755)


def prepare_esptool(target, dest, exe):
    system = target.split("-")[0]
    arch, checksum = ESPTOOL_ASSETS[target]
    suffix = "zip" if exe else "tar.gz"
    name = f"esptool-{ESPTOOL}-{system}-{arch}.{suffix}"
    url = f"https://github.com/espressif/esptool/releases/download/{ESPTOOL}/{name}"
    tool = "esptool" + exe
    executable(dest / tool, archive_files(url, {tool}, checksum)[tool])


def prepare_littlefs(target, dest, exe):
    system, arch = target.split("-")
    triple = {"macos": "apple-darwin", "windows": "w64-mingw32", "linux": "linux-gnu"}[system]
    suffix = "zip" if exe else "tar.gz"
    for tag, commit, tool in LITTLEFS:
        name = f"{arch}-{triple}-mklittlefs-{commit}.{suffix}"
        url = f"https://github.com/earlephilhower/mklittlefs/releases/download/{tag}/{name}"
        files = archive_files(url, {"mklittlefs" + exe, "libwinpthread-1.dll",
                                    "libgcc_s_seh-1.dll", "libstdc++-6.dll"})
        folder = dest / "littlefs-legacy" if exe and tool == "mklittlefs-legacy" else dest
        executable(folder / (tool + exe), files.pop("mklittlefs" + exe))
        for dll, data in files.items():
            save(folder / dll, data)


def prepare_mkspiffs(target, dest, exe):
    suffix, checksum = SPIFFS_ASSETS[target]
    name = f"mkspiffs-{SPIFFS}-arduino-esp32-{suffix}"
    url = f"https://github.com/igrr/mkspiffs/releases/download/{SPIFFS}/{name}"
    tool = "mkspiffs" + exe
    executable(dest / tool, archive_files(url, {tool}, checksum)[tool])


def idf_sources():
    sources = CACHE / f"esp-idf-{IDF}"
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        downloads = pool.map(lambda name: fetch(IDF_BASE + name), IDF_FILES)
        for name, data in zip(IDF_FILES, downloads):
            save(sources / name, data)
    return sources


def freeze_idf(sources, dest):
    subprocess.run([
        sys.executable, "-m", "PyInstaller", "--noconfirm", "--onefile", "--console",
        "--name", "idf-image", "--distpath", str(dest),
        "--workpath", str(CACHE / "pyinstaller" / HOST), "--specpath", str(CACHE),
        "--paths", str(sources / "components/spiffs"), "--paths", str(sources / "components/fatfs"),
        str(ROOT / "scripts/idf_image.py"),
    ], check=True)
    shutil.copyfile(Path(sysconfig.get_path("stdlib")) / "LICENSE.txt",
                    dest / "licenses/CPython.txt")
    shutil.copyfile(distribution_notice("pyinstaller", "COPYING.txt"),
                    dest / "licenses/PyInstaller.txt")


def distribution_notice(package, filename):
    distribution = importlib.metadata.distribution(package)
    return next(distribution.locate_file(name) for name in distribution.files
                if name.parts[0].endswith(".dist-info") and name.name == filename)


def write_idf_zipapp(sources):
    construct = importlib.metadata.distribution("construct")
    with tempfile.TemporaryDirectory(dir=SHARED) as work:
        path = Path(work) / "idf-image.pyz"
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.write(ROOT / "scripts/idf_image.py", "__main__.py")
            for name in IDF_FILES:
                relative = Path(*Path(name).parts[2:])
                archive.write(sources / name, relative.as_posix())
            for name in construct.files:
                if name.parts[0] == "construct" and name.suffix == ".py":
                    archive.write(construct.locate_file(name), name.as_posix())
            archive.write(distribution_notice("construct", "LICENSE"), "licenses/construct-MIT.txt")
        path.replace(SHARED / path.name)


def prepare_python(target, dest):
    triple, checksum = PYTHON_ASSETS[target]
    name = f"cpython-{PYTHON_VERSION}+{PYTHON_TAG}-{triple}-install_only_stripped.tar.gz"
    url = f"https://github.com/astral-sh/python-build-standalone/releases/download/{PYTHON_TAG}/{name}"
    data = fetch(url, checksum)
    windows = target.startswith("windows-")
    stdlib = "Lib" if windows else "lib/python3.12"
    skip = {"__pycache__", "config-3.12-darwin", "ensurepip", "idlelib", "site-packages",
            "test", "tests", "tkinter", "turtledemo", "venv"}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive:
            root, _, relative = member.name.partition("/")
            if root != "python" or not relative:
                continue
            parts = set(Path(relative).parts)
            # The unversioned Linux libpython link is for linking extensions.
            # The interpreter loads the versioned file, which also copies to FAT drives.
            if parts & skip or (member.issym() and relative == "lib/libpython3.12.so"):
                continue
            if (relative.startswith(("licenses/", "share/licenses/"))
                    or Path(relative).name.upper().startswith(("LICENSE", "COPYING", "NOTICE"))):
                archive.extract(member, dest, filter="data")
            elif (
                relative == "bin/python3.12" or relative == "python.exe"
                or relative.startswith((stdlib + "/", "DLLs/", "lib/libpython"))
                or (windows and "/" not in relative and relative.endswith(".dll"))
            ):
                archive.extract(member, dest, filter="data")


def prepare_licenses(dest, mkspiffs):
    licenses = dest / "licenses"
    files = {
        "esptool-GPL-2.0.txt": f"https://raw.githubusercontent.com/espressif/esptool/{ESPTOOL}/LICENSE",
        "ESP-IDF-APACHE-2.0.txt": IDF_BASE + "LICENSE",
        "mklittlefs-MIT.txt": "https://raw.githubusercontent.com/earlephilhower/mklittlefs/4.1.0/LICENSE.txt",
        "littlefs-BSD-3-Clause.txt": "https://raw.githubusercontent.com/littlefs-project/littlefs/v2.11.1/LICENSE.md",
        "littlefs-2.9.3-BSD-3-Clause.txt": "https://raw.githubusercontent.com/littlefs-project/littlefs/v2.9.3/LICENSE.md",
        "MPL-2.0.txt": "https://www.mozilla.org/media/MPL/2.0/index.815ca599c9df.txt",
    }
    if (dest / "libwinpthread-1.dll").is_file():
        files["winpthreads-COPYING.txt"] = (
            "https://raw.githubusercontent.com/mingw-w64/mingw-w64/v12.0.0/"
            "mingw-w64-libraries/winpthreads/COPYING"
        )
    if mkspiffs:
        files["mkspiffs-MIT.txt"] = f"https://raw.githubusercontent.com/igrr/mkspiffs/{SPIFFS}/LICENSE.txt"
        # The SPIFFS submodule license is maintained in its own repository.
        files["SPIFFS-MIT.txt"] = "https://raw.githubusercontent.com/pellepl/spiffs/f5e26c4/LICENSE"
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        for name, data in zip(files, pool.map(fetch, files.values())):
            save(licenses / name, data)
    shutil.copyfile(ROOT / "LICENSE", licenses / "ESP32-SuperFlash-GPL-3.0.txt")
    shutil.copyfile(ROOT / "assets/LICENSE-MaterialDesign.txt", licenses / "Pictogrammers.txt")
    shutil.copyfile(distribution_notice("construct", "LICENSE"), licenses / "construct-MIT.txt")
    for crate, version, checksum in (
        ("option-ext", "0.2.0", "04744f49eae99ab78e0d5c0b603ab218f515ea8cfe5a456d7629ad883a3b6e7d"),
        ("serialport", "4.10.1", "6ba5f8f29aa20853c4e3e85a33ec580eb66be1f057142e77a333834a318bacf2"),
    ):
        url = f"https://static.crates.io/crates/{crate}/{crate}-{version}.crate"
        notices = archive_files(url, {"LICENSE.txt"}, checksum)
        save(licenses / f"{crate}-MPL-2.0.txt", notices["LICENSE.txt"])


def prepare_fallback_font(script, column):
    name = f"NotoSans{script}"
    base = f"{GOOGLE_BASE}{name.lower()}/"
    font = TTFont(io.BytesIO(fetch(f"{base}{name}%5Bwght%5D.ttf")))
    if column is not None:
        # The OS supplies the complete typeface. Include a small fallback for the UI
        # and language picker on systems without this script's font installed.
        text = "简体中文繁體中文日本語한국어"
        for path in CATALOGUES:
            text += "".join(line.split("\t")[column] for line in path.read_text(encoding="utf-8").splitlines()
                            if line and not line.startswith("#"))
        options = subset.Options()
        options.name_IDs = ["*"]
        sub = subset.Subsetter(options=options)
        sub.populate(text=text)
        sub.subset(font)
    instantiateVariableFont(font, {"wght": 400}, inplace=True, updateFontNames=True)
    SHARED.mkdir(parents=True, exist_ok=True)
    font.save(SHARED / f"{name}.ttf")
    save(SHARED / f"OFL-{name}.txt", fetch(base + "OFL.txt"))


def fingerprint(paths):
    return hashlib.sha256(b"".join(path.read_bytes() for path in paths)).hexdigest()


def fresh(stamp, digest, expected):
    return stamp.is_file() and digest in stamp.read_text(encoding="utf-8") and all(
        path.is_file() for path in expected
    )


def prepare_shared(inputs):
    stamp = SHARED / "versions.txt"
    digest = fingerprint([*inputs, *CATALOGUES])
    expected = [SHARED / "idf-image.pyz"]
    for script in FALLBACK_FONTS:
        expected.extend([SHARED / f"NotoSans{script}.ttf", SHARED / f"OFL-NotoSans{script}.txt"])
    if not fresh(stamp, digest, expected):
        SHARED.mkdir(parents=True, exist_ok=True)
        write_idf_zipapp(idf_sources())
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            tasks = [pool.submit(prepare_fallback_font, script, column)
                     for script, column in FALLBACK_FONTS.items()]
            for task in tasks:
                task.result()
        stamp.write_text(digest + "\n", encoding="utf-8")


def validate_tools(dest, exe, image, mkspiffs):
    subprocess.run([str(dest / ("esptool" + exe)), "version"], check=True)
    for generator in ("spiffs", "fatfs", "fatfs-wl"):
        subprocess.run([*image, generator, "--help"], check=True, stdout=subprocess.DEVNULL)
    for _, _, tool in LITTLEFS:
        folder = dest / "littlefs-legacy" if exe and tool == "mklittlefs-legacy" else dest
        subprocess.run([str(folder / (tool + exe)), "--version"], check=True)
    if mkspiffs:
        subprocess.run([str(dest / ("mkspiffs" + exe)), "--version"], check=True)


def prepare_platform(target, digest):
    dest = TOOLS / target
    stamp = dest / "versions.txt"
    exe = ".exe" if target.startswith("windows-") else ""
    mkspiffs = target in SPIFFS_ASSETS
    # Only the current Apple Silicon build uses a frozen helper. All other
    # platforms use a released interpreter with the same generator sources.
    frozen = target == HOST == "macos-aarch64"
    runtime = "python/python.exe" if exe else "python/bin/python3.12"
    image_program = "idf-image" if frozen else runtime
    names = ["esptool" + exe, "mklittlefs" + exe, "mklittlefs-legacy" + exe]
    if exe:
        names[-1] = "littlefs-legacy/" + names[-1]
        names.extend(["libwinpthread-1.dll", "littlefs-legacy/libwinpthread-1.dll"])
    if target == "macos-aarch64" and not frozen and fresh(
        stamp, digest, [dest / name for name in [*names, "idf-image"]]
    ):
        print(f"Using tools in {dest}", flush=True)
        return
    names.append(image_program)
    if mkspiffs:
        names.append("mkspiffs" + exe)
    if fresh(stamp, digest, [dest / name for name in names]):
        print(f"Using tools in {dest}", flush=True)
        return
    with tempfile.TemporaryDirectory(dir=TOOLS, prefix=f".{target}-") as work:
        stage = Path(work) / "payload"
        stage.mkdir()
        prepare_esptool(target, stage, exe)
        prepare_littlefs(target, stage, exe)
        if mkspiffs:
            prepare_mkspiffs(target, stage, exe)
        prepare_licenses(stage, mkspiffs)
        if frozen:
            freeze_idf(idf_sources(), stage)
            image = [str(stage / image_program)]
        else:
            prepare_python(target, stage)
            image = [str(stage / image_program), "-I", "-B", "-X", "utf8", str(SHARED / "idf-image.pyz")]
        if target == HOST:
            validate_tools(stage, exe, image, mkspiffs)
        versions = [
            f"esptool {ESPTOOL}", f"ESP-IDF {IDF}",
            "mklittlefs 4.1.0 (LittleFS 2.11.1)", "mklittlefs 4.0.2 (LittleFS 2.9.3)",
            "idf-image (standalone)" if frozen else f"idf-image.pyz (CPython {PYTHON_VERSION})",
        ]
        if mkspiffs:
            versions.append(f"mkspiffs {SPIFFS} (Arduino ESP32)")
        (stage / stamp.name).write_text("\n".join([*versions, digest, ""]), encoding="utf-8")
        previous = Path(work) / "previous"
        if dest.exists():
            dest.rename(previous)
        try:
            stage.rename(dest)
        except OSError:
            if previous.exists():
                previous.rename(dest)
            raise
        print(f"Prepared tools in {dest}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--all", action="store_true", help="prepare every supported platform")
    args = parser.parse_args()
    if HOST not in ESPTOOL_ASSETS:
        parser.error(f"Unsupported build host: {HOST}")
    targets = list(ESPTOOL_ASSETS) if args.all else [HOST]
    inputs = [Path(__file__), ROOT / "scripts/idf_image.py", ROOT / "scripts/build-requirements.txt"]
    prepare_shared(inputs)
    digest = fingerprint(inputs)
    for target in targets:
        prepare_platform(target, digest)


if __name__ == "__main__":
    main()
