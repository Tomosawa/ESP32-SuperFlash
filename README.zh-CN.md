# ESP32 SuperFlash

[English](README.md) | 简体中文

通过串口烧录 ESP32 固件，并生成 LittleFS、FatFS 和 SPIFFS 数据分区镜像。发布包内置 esptool 和镜像生成工具，可离线使用。

## 下载与安装

[从 GitHub 下载](https://github.com/Tomosawa/ESP32-SuperFlash/releases) · Windows x64、macOS 14 及以上（Apple Silicon 和 Intel）、Linux（x64 和 ARM64）

- **Windows** 运行安装程序，或解压便携版 ZIP 后打开 `ESP32-SuperFlash.exe`。
- **macOS** 打开 DMG，将 SuperFlash 拖到「应用程序」。
- **Linux** 解压后运行 `./ESP32-SuperFlash`，保留同级的 `tools` 目录。如果无法打开串口，检查当前用户是否有串口访问权限。

## 烧录模式

### 完整固件

烧录已合并的 RAW 二进制镜像，起始地址默认为 `0x000000`。

![使用合并镜像烧录完整固件](assets/screenshots/full_flash_zh-CN.png)

### 多分区

导入 UTF-8 编码的 `partition.csv` 或手动填写地址。可烧录选定的 `.bin` 文件，也可合并导出为 RAW、Intel HEX 或 UF2。

![带有烧录地址和二进制文件的分区表](assets/screenshots/multi_partition_zh-CN.png)

**分区偏移。** CSV 中留空的偏移按 ESP-IDF 对齐规则计算。分区表默认位于 `0x8000`。如果固件修改了 `CONFIG_PARTITION_TABLE_OFFSET`，请填写实际值。芯片设为自动识别时，含有 `bootloader/primary` 的 CSV 需要手动填写主 Bootloader 偏移。

**合并镜像。** 合并前请选择目标芯片。导出 RAW 时如果指定了 `--target-offset`，烧录该镜像时请使用同一地址。HEX 和 UF2 用于导出。

### 数据分区

从本地文件夹生成 LittleFS、FatFS 或 SPIFFS 镜像，可保存到本地或直接烧录。

选择「原始镜像」可直接写入已有文件，例如固件直接读取的 `.tar`。容量和偏移地址以目标数据分区为准。

![LittleFS 分区设置和文件预览](assets/screenshots/data_partition_zh-CN.png)

**文件系统镜像。** 分区大小和地址以固件的分区表为准。镜像参数也要和固件匹配。

- **LittleFS** 可选 2.9.3、2.11.1 或自备的 `mklittlefs`。块大小和页大小需与固件一致。
- **FatFS** 需匹配裸 FAT 或磨损均衡布局、扇区大小、512 字节安全或性能模式以及长文件名配置。
- **SPIFFS** 需匹配页大小、对象名和元数据长度、Magic 标志及索引对齐设置。

## 从源码构建

安装 Rust 1.88 或更新版本和 [uv](https://docs.astral.sh/uv/getting-started/installation/)，在仓库根目录运行以下命令。

```sh
uv run --python 3.12 --with-requirements scripts/build-requirements.txt python scripts/fetch-tools.py
cargo run --locked
```

在对应平台运行打包脚本，产物位于 `dist/`。

| 平台 | 命令 | 构建依赖 |
| --- | --- | --- |
| macOS | `./scripts/package-macos.sh` | Xcode 命令行工具 |
| Windows | `powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1` | Rust MSVC 工具链和 Visual Studio C++ Build Tools。生成安装包还需要 Inno Setup 6.5 或更新版本。 |
| Linux | `./scripts/package-linux.sh` | Ubuntu 22.04 及以上安装 `build-essential pkg-config libudev-dev libxkbcommon-dev libgl1-mesa-dev fontconfig`。 |

## 问题反馈与贡献

遇到烧录问题时，请在 [GitHub Issues](https://github.com/Tomosawa/ESP32-SuperFlash/issues) 附上 SuperFlash 版本、操作系统、ESP32 型号和错误日志。

修复和改进可通过 Pull Request 提交。

## 许可证

[GPL-3.0-only](LICENSE)。组件版本、许可证和源码链接见[随附工具和第三方版权说明](tools/README.md)。
