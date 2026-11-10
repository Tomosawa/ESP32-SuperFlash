use crate::{
    engine::{self, JobContext, Tools},
    model::{self, Connection, DataConfig, FileSystem, Project, Segment},
};
use anyhow::Result;
use std::{
    fs,
    sync::{Arc, atomic::AtomicBool, mpsc},
};

#[test]
fn idf_csv_import_aligns_blank_offsets_and_preserves_selected_files() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let csv = dir.path().join("partition.csv");
    fs::write(
        &csv,
        "\u{feff}# partition map\nbootloader,bootloader,primary,N/A,N/A\npartition_table,partition_table,primary,N/A,N/A\nnvs,data,nvs,,24K\nfactory,app,factory,,1M\ndata,0x01,0x83,,-2M,\n",
    )?;
    let mut project = Project {
        connection: Connection {
            chip: "esp32".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut rows = model::read_partitions(&csv, &project)?;
    let values: Vec<_> = rows.iter().map(|row| (&*row.address, &*row.size)).collect();
    assert_eq!(
        values,
        [
            ("0x001000", "0x007000"),
            ("0x008000", "0x001000"),
            ("0x009000", "0x006000"),
            ("0x010000", "0x100000"),
            ("0x110000", "0x0F0000"),
        ]
    );
    assert!(model::data_partition(FileSystem::LittleFs, &rows[4].kind));
    rows[3].path = "app.bin".into();
    rows[3].enabled = true;
    project.segments = rows;
    let rows = model::read_partitions(&csv, &project)?;
    assert_eq!(rows[3].path, "app.bin");
    assert!(rows[3].enabled);
    Ok(())
}

#[test]
fn write_checks_sector_overlap_capacity_and_advanced_argument_boundaries() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let binary = dir.path().join("image file.bin");
    fs::write(&binary, [0x42; 40])?;
    let first = Segment {
        size: "1K".into(),
        path: binary.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let mut second = Segment {
        address: "0x0100".into(),
        ..first.clone()
    };
    let mut options = Connection {
        port: "/dev/nonexistent".into(),
        ..Default::default()
    };
    assert!(model::inputs([&first, &second], &options, true).is_err());
    second.address = "0x1000".into();
    second.encrypt = true;
    let inputs = model::inputs([&first, &second], &options, true)?;
    let tools = Tools::locate();
    options.write_options = "--diff-with\nold image.bin\nskip".into();
    let command = engine::flash_command(&tools, &options, &inputs, false)?.display();
    assert!(command.contains("'--diff-with' 'old image.bin' 'skip' '--encrypt-files'"));
    assert!(command.contains("'--' '0x000000'"));
    options.write_options = "--erase-all".into();
    assert!(engine::flash_command(&tools, &options, &inputs, true).is_err());
    options.merge_options = "--output\nother.bin".into();
    options.chip = "esp32".into();
    assert!(engine::merge_command(&tools, &options, &inputs, "raw", &binary).is_err());
    assert!(engine::save_output(&binary, &inputs, |_| panic!("output aliases input")).is_err());
    Ok(())
}

#[test]
#[ignore = "requires platform-native tools prepared by the packaging script"]
fn bundled_tools_generate_seven_layouts_and_merge_without_hardware() -> Result<()> {
    let tools = Tools::locate();
    assert!(tools.available(), "prepare the platform tools first");
    let (tx, rx) = mpsc::channel();
    let ctx = JobContext {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
        repaint: eframe::egui::Context::default(),
    };
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("data folder");
    fs::create_dir_all(source.join("sub/empty"))?;
    let marker = b"SuperFlash portable test 0123456789.";
    fs::write(source.join("sub/start.txt"), marker)?;
    fs::write(source.join("人物.txt"), b"index")?;
    fs::write(source.join(".DS_Store"), b"should be excluded")?;
    let config = DataConfig {
        folder: source.to_string_lossy().into_owned(),
        size: "1M".into(),
        ..Default::default()
    };
    let layouts = [
        ("littlefs-2.9", FileSystem::LittleFs, true, "wl", "4096"),
        ("littlefs-2.11", FileSystem::LittleFs, false, "wl", "4096"),
        ("spiffs", FileSystem::Spiffs, false, "wl", "4096"),
        ("fat-wl", FileSystem::FatFs, false, "wl", "4096"),
        ("fat-safe", FileSystem::FatFs, false, "safe", "512"),
        ("fat-perf", FileSystem::FatFs, false, "wl", "512"),
        ("fat-raw", FileSystem::FatFs, false, "raw", "4096"),
    ];
    for (name, format, legacy, layout, sector) in layouts {
        let config = DataConfig {
            format,
            littlefs_legacy: legacy,
            fat_layout: layout.into(),
            fat_sector: sector.into(),
            ..config.clone()
        };
        let output = dir.path().join(format!("{name}.bin"));
        if let Err(error) = engine::generate_image(&tools, &config, &output, &ctx) {
            for event in rx.try_iter() {
                if let engine::Event::Log(line) = event {
                    eprintln!("{line}");
                }
            }
            panic!("{name}: {error:#}");
        }
        let bytes = fs::read(output)?;
        assert_eq!(bytes.len(), 1024 * 1024, "{name}");
        assert!(
            bytes.windows(marker.len()).any(|bytes| bytes == marker),
            "{name}"
        );
        assert!(
            !bytes
                .windows(18)
                .any(|bytes| bytes == b"should be excluded"),
            "{name}"
        );
        if format == FileSystem::FatFs {
            let sector = if layout == "raw" { 0 } else { 4096 };
            assert_eq!(&bytes[sector + 510..sector + 512], &[0x55, 0xAA], "{name}");
            let filename: Vec<u8> = "人物.txt"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            // LFN records split the name after the fifth UTF-16 code unit.
            assert!(
                bytes.windows(10).any(|part| part == &filename[..10]),
                "{name}"
            );
            if layout != "raw" {
                let config = bytes.len() - 4096;
                let partition_size = u32::from_le_bytes(bytes[config + 4..config + 8].try_into()?);
                assert_eq!(partition_size, bytes.len() as u32);
            }
        }
    }
    let first = dir.path().join("a.bin");
    let second = dir.path().join("b.bin");
    fs::write(&first, b"boot")?;
    fs::write(&second, b"app")?;
    let options = Connection {
        chip: "esp32".into(),
        ..Default::default()
    };
    let segments = [
        Segment {
            path: first.to_string_lossy().into_owned(),
            address: "0x1000".into(),
            ..Default::default()
        },
        Segment {
            path: second.to_string_lossy().into_owned(),
            address: "0x10000".into(),
            ..Default::default()
        },
    ];
    let inputs = model::inputs(&segments, &options, true)?;
    let merged = dir.path().join("merged.bin");
    engine::save_output(&merged, &inputs, |tmp| {
        engine::merge_command(&tools, &options, &inputs, "raw", tmp)?.run(&ctx)
    })?;
    let bytes = fs::read(merged)?;
    assert!(bytes[..0x1000].iter().all(|&byte| byte == 0xff));
    assert_eq!(&bytes[0x1000..0x1004], b"boot");
    assert_eq!(&bytes[0x10000..], b"app");
    Ok(())
}
