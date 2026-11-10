use crate::i18n::{message, t};
use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Firmware,
    Partitions,
    Data,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSystem {
    #[default]
    LittleFs,
    FatFs,
    Spiffs,
}

impl FileSystem {
    pub fn name(self) -> &'static str {
        match self {
            Self::LittleFs => "LittleFS",
            Self::FatFs => "FatFS",
            Self::Spiffs => "SPIFFS",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub chip: String,
    pub port: String,
    pub auto_port: bool,
    pub baud: String,
    pub flash_mode: String,
    pub flash_size: String,
    pub flash_freq: String,
    pub before: String,
    pub after: String,
    pub compression: String,
    pub no_stub: bool,
    pub erase_all: bool,
    pub encrypt: bool,
    pub trace: bool,
    pub globals: String,
    pub write_options: String,
    pub merge_options: String,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            chip: "auto".into(),
            port: String::new(),
            auto_port: false,
            baud: "115200".into(),
            flash_mode: "keep".into(),
            flash_size: "keep".into(),
            flash_freq: "keep".into(),
            before: "default-reset".into(),
            after: "hard-reset".into(),
            compression: "default".into(),
            no_stub: false,
            erase_all: false,
            encrypt: false,
            trace: false,
            globals: String::new(),
            write_options: String::new(),
            merge_options: String::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Segment {
    pub enabled: bool,
    pub encrypt: bool,
    pub name: String,
    pub kind: String,
    pub address: String,
    pub size: String,
    pub path: String,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            enabled: true,
            encrypt: false,
            name: String::new(),
            kind: String::new(),
            address: "0x000000".into(),
            size: String::new(),
            path: String::new(),
        }
    }
}

pub const DEFAULT_DATA_ADDRESS: &str = "0x290000";
pub const DEFAULT_DATA_SIZE: &str = "1M";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataConfig {
    pub folder: String,
    pub address: String,
    pub size: String,
    pub format: FileSystem,
    pub block: String,
    pub page: String,
    pub littlefs_page: String,
    pub littlefs_legacy: bool,
    pub custom_littlefs: String,
    pub ignore_system: bool,
    pub fat_layout: String,
    pub fat_sector: String,
    pub fat_lfn: bool,
    pub fat_clusters: String,
    pub fat_count: String,
    pub spiffs_name: String,
    pub spiffs_meta: String,
    pub spiffs_magic: bool,
    pub spiffs_magic_len: bool,
    pub spiffs_aligned: bool,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            folder: String::new(),
            address: DEFAULT_DATA_ADDRESS.into(),
            size: DEFAULT_DATA_SIZE.into(),
            format: FileSystem::LittleFs,
            block: "4096".into(),
            page: "256".into(),
            littlefs_page: "256".into(),
            littlefs_legacy: true,
            custom_littlefs: String::new(),
            ignore_system: true,
            fat_layout: "wl".into(),
            fat_sector: "4096".into(),
            fat_lfn: true,
            fat_clusters: "1".into(),
            fat_count: "2".into(),
            spiffs_name: "32".into(),
            spiffs_meta: "4".into(),
            spiffs_magic: true,
            spiffs_magic_len: true,
            spiffs_aligned: false,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub mode: Mode,
    pub firmware: Segment,
    pub segments: Vec<Segment>,
    pub data: DataConfig,
    pub connection: Connection,
    pub csv_path: String,
    pub table_offset: String,
    pub boot_offset: String,
    pub recovery_offset: String,
    pub merge_path: String,
    pub merge_format: String,
    pub image_path: String,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            mode: Mode::Firmware,
            firmware: Segment::default(),
            segments: Vec::new(),
            data: DataConfig::default(),
            connection: Connection::default(),
            csv_path: String::new(),
            table_offset: "0x8000".into(),
            boot_offset: String::new(),
            recovery_offset: String::new(),
            merge_path: String::new(),
            merge_format: "raw".into(),
            image_path: String::new(),
        }
    }
}

pub fn read_project(path: &Path) -> Result<Project> {
    let bytes = fs::read(path).with_context(|| message("read_failed", &[&path.display()]))?;
    let mut project: Project = serde_json::from_slice(&bytes).context(t("invalid_project"))?;
    // Older workspaces saved an unused data form with both fields empty.
    let data = &mut project.data;
    if data.folder.is_empty() && data.address.is_empty() && data.size.is_empty() {
        data.address = DEFAULT_DATA_ADDRESS.into();
        data.size = DEFAULT_DATA_SIZE.into();
    }
    Ok(project)
}

pub fn save_project(path: &Path, project: &Project) -> Result<()> {
    save_json(path, project)
}

pub fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context(t("invalid_path"))?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut tmp, value)?;
    writeln!(tmp)?;
    tmp.persist(path)
        .with_context(|| message("save_failed", &[&path.display()]))?;
    Ok(())
}

pub fn number(input: &str) -> Result<u64> {
    let text = input.trim().to_ascii_lowercase();
    let text = text.strip_prefix('+').unwrap_or(&text);
    let (digits, multiple) = ["mib", "mb", "m", "kib", "kb", "k"]
        .into_iter()
        .find_map(|suffix| {
            text.strip_suffix(suffix).map(|digits| {
                (
                    digits.trim(),
                    if suffix.starts_with('m') {
                        1 << 20
                    } else {
                        1 << 10
                    },
                )
            })
        })
        .unwrap_or((text, 1));
    let parsed = if let Some(hex) = digits.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else if let Some(bin) = digits.strip_prefix("0b") {
        u64::from_str_radix(bin, 2)
    } else if let Some(oct) = digits.strip_prefix("0o") {
        u64::from_str_radix(oct, 8)
    } else {
        digits.parse()
    };
    parsed
        .ok()
        .and_then(|value| value.checked_mul(multiple))
        .ok_or_else(|| anyhow!(message("invalid_number", &[&input])))
}

pub fn address(input: &str) -> Result<u64> {
    let value = number(input)?;
    ensure!(u32::try_from(value).is_ok(), t("invalid_address"));
    Ok(value)
}

pub fn hex(value: u64) -> String {
    format!("0x{value:06X}")
}

pub fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

pub fn read_partitions(path: &Path, project: &Project) -> Result<Vec<Segment>> {
    let csv = fs::read_to_string(path).context(t("csv_read"))?;
    let table = address(&project.table_offset).context(t("invalid_address"))?;
    ensure!(table % 4096 == 0, t("align_4k"));
    let mut next = table + 4096;
    let mut names = HashSet::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .from_reader(csv.trim_start_matches('\u{feff}').as_bytes());
    let mut result = Vec::new();
    for record in reader.records() {
        let row = record?;
        if row
            .get(0)
            .is_none_or(|s| s.is_empty() || s.starts_with('#'))
        {
            continue;
        }
        if row.get(0).is_some_and(|s| s.eq_ignore_ascii_case("name"))
            && row.get(1).is_some_and(|s| s.eq_ignore_ascii_case("type"))
        {
            continue;
        }
        let parse = || -> Result<(Segment, bool)> {
            ensure!((5..=6).contains(&row.len()), t("csv_columns"));
            let (name, kind, sub) = (
                &row[0],
                row[1].to_ascii_lowercase(),
                row[2].to_ascii_lowercase(),
            );
            let kind_num = match kind.as_str() {
                "app" => 0,
                "data" => 1,
                "bootloader" => 2,
                "partition_table" => 3,
                _ => number(&kind).context(t("csv_type"))?,
            };
            ensure!(kind_num <= 0xfe, t("csv_type"));
            let app = kind_num == 0;
            let boot = kind_num == 2;
            let part = kind_num == 3;
            ensure!(!app || !sub.is_empty(), t("csv_subtype"));
            let subtype_num = number(&sub).ok();
            let primary = sub == "primary" || subtype_num == Some(0);
            let recovery = sub == "recovery" || subtype_num == Some(2);
            let alignment = if app { 65536 } else { 4096 };
            let boot_base = || -> Result<u64> {
                if !project.boot_offset.trim().is_empty() {
                    return address(&project.boot_offset);
                }
                match project.connection.chip.as_str() {
                    "esp32" | "esp32s2" => Ok(0x1000),
                    "esp32p4" | "esp32c5" | "esp32h4" | "esp32s31" => Ok(0x2000),
                    "auto" | "" | "esp8266" => bail!(t("csv_boot")),
                    _ => Ok(0),
                }
            };
            let offset = if boot && primary {
                boot_base()?
            } else if boot && recovery {
                address(&project.recovery_offset).context(t("csv_recovery"))?
            } else if part && primary {
                table
            } else if row[3].is_empty() {
                next.div_ceil(alignment) * alignment
            } else {
                address(&row[3])?
            };
            let size = if boot {
                table
                    .checked_sub(boot_base()?)
                    .context(t("csv_boot_order"))?
            } else if part {
                4096
            } else if let Some(end) = row[4].strip_prefix('-') {
                number(end)?
                    .checked_sub(offset)
                    .context(t("invalid_address"))?
            } else {
                number(&row[4])?
            };
            ensure!(
                size > 0 && offset % alignment == 0,
                message("csv_alignment", &[&human_size(alignment)])
            );
            let primary_reserved = (boot || part) && primary;
            if !primary_reserved {
                ensure!(offset >= next, message("csv_order", &[&hex(next)]));
            }
            ensure!(
                offset
                    .checked_add(size)
                    .is_some_and(|end| end <= u32::MAX as u64 + 1),
                t("invalid_address")
            );
            let existing = project
                .segments
                .iter()
                .find(|entry| entry.name == name && address(&entry.address).ok() == Some(offset));
            let segment = Segment {
                name: name.into(),
                kind: if sub.is_empty() {
                    kind
                } else {
                    format!("{kind}/{sub}")
                },
                address: hex(offset),
                size: hex(size),
                enabled: existing.is_some_and(|entry| entry.enabled),
                encrypt: existing.is_some_and(|entry| entry.encrypt),
                path: existing.map_or_else(String::new, |entry| entry.path.clone()),
            };
            Ok((segment, primary_reserved))
        };
        let (entry, primary_reserved) = parse().with_context(|| {
            message(
                "csv_line",
                &[&row.position().map_or(0, csv::Position::line)],
            )
        })?;
        ensure!(
            names.insert(entry.name.clone()),
            message("csv_duplicate", &[&entry.name])
        );
        if !primary_reserved {
            next = address(&entry.address)? + number(&entry.size)?;
        }
        result.push(entry);
    }
    ensure!(!result.is_empty(), t("csv_empty"));
    let mut ranges = result
        .iter()
        .map(|entry| Ok((address(&entry.address)?, number(&entry.size)?)))
        .collect::<Result<Vec<_>>>()?;
    ranges.sort_unstable();
    ensure!(
        ranges
            .windows(2)
            .all(|pair| pair[0].0 + pair[0].1 <= pair[1].0),
        t("csv_overlap")
    );
    Ok(result)
}

#[derive(Debug)]
pub struct WriteInput {
    pub address: u64,
    pub path: PathBuf,
    pub size: u64,
    pub encrypt: bool,
}

pub fn inputs<'a>(
    segments: impl IntoIterator<Item = &'a Segment>,
    connection: &Connection,
    flashing: bool,
) -> Result<Vec<WriteInput>> {
    let mut inputs = Vec::new();
    let capacity = number(connection.flash_size.trim_end_matches("-c1")).ok();
    for segment in segments.into_iter().filter(|segment| segment.enabled) {
        let path = PathBuf::from(&segment.path);
        let display = if segment.name.is_empty() {
            &segment.address
        } else {
            &segment.name
        };
        let meta = fs::metadata(&path).with_context(|| message("invalid_binary", &[&display]))?;
        ensure!(
            meta.is_file() && meta.len() != 0,
            message("invalid_binary", &[&display])
        );
        let mut prefix = [0u8; 16];
        let n = std::io::Read::read(&mut fs::File::open(&path)?, &mut prefix)?;
        let hex_input =
            n >= 11 && prefix[0] == b':' && prefix[1..11].iter().all(u8::is_ascii_hexdigit);
        let uf2_input = prefix.starts_with(b"UF2\nWQ]\x9e");
        ensure!(
            !hex_input && !uf2_input,
            message("invalid_binary", &[&display])
        );
        let offset = address(&segment.address).with_context(|| t("invalid_address"))?;
        if !segment.size.trim().is_empty() {
            let capacity = number(&segment.size)?;
            ensure!(
                meta.len() <= capacity,
                message(
                    "segment_size",
                    &[&display, &human_size(meta.len()), &human_size(capacity)]
                )
            );
        }
        let end = offset
            .checked_add(meta.len())
            .filter(|&n| n <= u32::MAX as u64 + 1)
            .context(t("invalid_address"))?;
        let end = if flashing {
            end.div_ceil(4096) * 4096
        } else {
            end
        };
        ensure!(
            capacity.is_none_or(|bytes| end <= bytes),
            t("flash_capacity")
        );
        inputs.push(WriteInput {
            address: offset,
            path,
            size: meta.len(),
            encrypt: segment.encrypt,
        });
    }
    ensure!(!inputs.is_empty(), t("no_inputs"));
    inputs.sort_by_key(|segment| segment.address);
    for pair in inputs.windows(2) {
        let end = pair[0].address + pair[0].size;
        let overlap = if flashing {
            end.div_ceil(4096) > pair[1].address / 4096
        } else {
            end > pair[1].address
        };
        ensure!(
            !overlap,
            message(
                "write_overlap",
                &[&hex(pair[0].address), &hex(pair[1].address)]
            )
        );
    }
    Ok(inputs)
}

pub fn data_partition(format: FileSystem, kind: &str) -> bool {
    let Some((kind, sub)) = kind.split_once('/') else {
        return false;
    };
    if kind != "data" && number(kind).ok() != Some(1) {
        return false;
    }
    match format {
        FileSystem::LittleFs => {
            matches!(sub, "littlefs" | "spiffs") || matches!(number(sub).ok(), Some(0x82 | 0x83))
        }
        FileSystem::FatFs => sub == "fat" || number(sub).ok() == Some(0x81),
        FileSystem::Spiffs => sub == "spiffs" || number(sub).ok() == Some(0x82),
    }
}
