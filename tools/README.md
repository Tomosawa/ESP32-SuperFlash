# Bundled tools

Each application package includes only its matching platform directory and `shared`. The shared directory contains the filesystem generator archive and fallback fonts.

| Directory | Flash and LittleFS | SPIFFS and FatFS used by SuperFlash | Additional SPIFFS utility |
| --- | --- | --- | --- |
| `macos-aarch64` | esptool, both mklittlefs versions | Standalone `idf-image` | ESP-IDF generator included in `idf-image` |
| `macos-x86_64` | esptool, both mklittlefs versions | Bundled CPython + `shared/idf-image.pyz` | `mkspiffs` (Intel) |
| `windows-x86_64` | esptool.exe, both mklittlefs versions with their DLLs | Bundled CPython + `shared/idf-image.pyz` | `mkspiffs.exe` (32-bit upstream release for Windows) |
| `linux-x86_64` | esptool, both mklittlefs versions | Bundled CPython + `shared/idf-image.pyz` | `mkspiffs` (Linux x64) |
| `linux-aarch64` | esptool, both mklittlefs versions | Bundled CPython + `shared/idf-image.pyz` | ESP-IDF generator included in the archive |

The ESP-IDF generators support SPIFFS options that the published mkspiffs executable fixes at build time. SuperFlash uses the ESP-IDF generators to apply the requested settings. It also uses ESP-IDF's FatFS generators for raw and wear-levelled images, including the 512-byte safety and performance layouts. The [labplus-cn mkfatfs releases](https://github.com/labplus-cn/mkfatfs/releases) are available for fixed-format workflows.

## Third-party notices and source links

The components below retain their respective upstream licenses. Unless a different location is listed, the notice file is in `tools/<platform>/licenses/` in the application package.

| Component and upstream source | Copyright / attribution | License and packaged notice |
| --- | --- | --- |
| [esptool 5.6.0 source](https://github.com/espressif/esptool/tree/v5.6.0) and [release binaries](https://github.com/espressif/esptool/releases/tag/v5.6.0) | Fredrik Ahlberg, Angus Gratton, Espressif Systems (Shanghai) CO LTD, and contributors | GPL-2.0-or-later · `esptool-GPL-2.0.txt` |
| mklittlefs [4.1.0](https://github.com/earlephilhower/mklittlefs/releases/tag/4.1.0) and [4.0.2](https://github.com/earlephilhower/mklittlefs/releases/tag/4.0.2) | Ivan Grokhotkov | MIT · `mklittlefs-MIT.txt` |
| LittleFS [2.11.1](https://github.com/littlefs-project/littlefs/tree/v2.11.1) and [2.9.3](https://github.com/littlefs-project/littlefs/tree/v2.9.3) | The littlefs authors and Arm Limited | BSD-3-Clause · `littlefs-BSD-3-Clause.txt` and `littlefs-2.9.3-BSD-3-Clause.txt` |
| [mkspiffs 0.2.3 for Arduino ESP32](https://github.com/igrr/mkspiffs/releases/tag/0.2.3) and its [SPIFFS library](https://github.com/pellepl/spiffs/tree/f5e26c4) | Ivan Grokhotkov (mkspiffs); Peter Andersson (SPIFFS) | MIT · `mkspiffs-MIT.txt` and `SPIFFS-MIT.txt`, where bundled |
| ESP-IDF 6.1 [FatFS generators](https://github.com/espressif/esp-idf/tree/v6.1/components/fatfs) and [SPIFFS generator](https://github.com/espressif/esp-idf/blob/v6.1/components/spiffs/spiffsgen.py) in `idf-image` / `idf-image.pyz` | Espressif Systems (Shanghai) CO LTD | Apache-2.0 · `ESP-IDF-APACHE-2.0.txt` |
| [Construct 2.10.70](https://pypi.org/project/construct/2.10.70/#files) in the image helper | Arkadiusz Bulski, Tomer Filiba, and Corbin Simpson | MIT · `construct-MIT.txt` |
| [CPython 3.12](https://github.com/python/cpython/tree/3.12) and the portable [Python Build Standalone release](https://github.com/astral-sh/python-build-standalone/releases/tag/20261026) | Python Software Foundation and contributors; see the runtime's full notice for other copyright holders | PSF license and historical terms · `CPython.txt` for the standalone helper; `LICENSE.txt` under `tools/<platform>/python/` for the portable runtime |
| [PyInstaller 6.22.3](https://github.com/pyinstaller/pyinstaller/tree/v6.22.3), used to bundle the standalone image helper | PyInstaller Development Team, Giovanni Bajo, and contributors | [GPL 2.0 with bundling exception; Apache-2.0 for specified files](https://pyinstaller.org/en/stable/license.html) · `PyInstaller.txt` where the standalone helper is bundled |
| [MinGW-w64 winpthreads](https://github.com/mingw-w64/mingw-w64/tree/v12.0.0/mingw-w64-libraries/winpthreads) (`libwinpthread-1.dll` included in the Windows mklittlefs archives) | MinGW-w64 project and contributors | See `winpthreads-COPYING.txt` |
| [Noto Sans CJK](https://github.com/notofonts/noto-cjk), fallback fonts | Adobe | SIL Open Font License 1.1 · `tools/shared/OFL-NotoSans*.txt` |
| [Material Design Icons: GitHub](https://pictogrammers.com/library/mdi/icon/github/) | Pictogrammers | Apache-2.0 · `Pictogrammers.txt` |
| [serialport 4.10.1](https://docs.rs/crate/serialport/4.10.1/source/) and [option-ext 0.2.0](https://docs.rs/crate/option-ext/0.2.0/source/), libraries compiled into the application | Their respective authors and contributors | `MPL-2.0.txt`; upstream notices in `serialport-MPL-2.0.txt` and `option-ext-MPL-2.0.txt` |

When publishing a package that includes esptool, provide the corresponding source as required by [section 3 of the GPL v2](https://github.com/espressif/esptool/blob/v5.6.0/LICENSE). The attribution and upstream links here identify the components; they do not replace that distribution requirement.

Run `uv run --python 3.12 --with-requirements scripts/build-requirements.txt python scripts/fetch-tools.py --all` from the project root to restore the resources. Without `--all`, the script prepares the current platform.
