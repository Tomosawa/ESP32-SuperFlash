use crate::{engine::Tools, i18n::Language};
use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use std::{path::PathBuf, sync::Arc};

pub fn install(ctx: &Context, tools: &Tools, language: Language) {
    let mut fonts = FontDefinitions::default();
    let mut system_fonts = Vec::new();
    if let Some((path, index)) = system_font(language) {
        system_fonts.push((path, index));
    }
    for (name, script) in [
        ("NotoSansTC.ttf", Language::ZhHant),
        ("NotoSansJP.ttf", Language::Ja),
        ("NotoSansKR.ttf", Language::Ko),
        ("NotoSansSC.ttf", Language::ZhHans),
    ] {
        if script == language {
            system_fonts.push((tools.shared.join(name), 0));
        }
    }
    // Keep all language names readable in the language menu, even on minimal installations.
    system_fonts.extend(
        [
            "NotoSansSC.ttf",
            "NotoSansTC.ttf",
            "NotoSansJP.ttf",
            "NotoSansKR.ttf",
        ]
        .map(|name| (tools.shared.join(name), 0)),
    );
    let mut seen = std::collections::HashSet::new();
    let monospace_len = fonts.families[&FontFamily::Monospace].len();
    for (path, index) in system_fonts
        .into_iter()
        .filter(|pair| seen.insert(pair.clone()))
        .rev()
    {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if ttf_parser::Face::parse(&bytes, index).is_err() {
            continue;
        }
        let name = format!("{}#{index}", path.display());
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        fonts.font_data.insert(name.clone(), Arc::new(data));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, name.clone());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(monospace_len, name);
    }
    ctx.set_fonts(fonts);
}

#[cfg(target_os = "macos")]
fn system_font(language: Language) -> Option<(PathBuf, u32)> {
    let requested = match language {
        Language::ZhHant => "PingFangTC-Regular",
        Language::Ja => "HiraginoSans-W3",
        Language::Ko => "AppleSDGothicNeo-Regular",
        _ => "PingFangSC-Regular",
    };
    let font = core_text::font::new_from_name(requested, 14.0).ok()?;
    let name = font.postscript_name();
    if name != requested {
        return None;
    }
    let path = font.url()?.to_path()?;
    let data = std::fs::read(&path).ok()?;
    let index = (0..ttf_parser::fonts_in_collection(&data).unwrap_or(1)).find(|&index| {
        ttf_parser::Face::parse(&data, index).is_ok_and(|face| {
            face.names().into_iter().any(|entry| {
                entry.name_id == ttf_parser::name_id::POST_SCRIPT_NAME
                    && entry.to_string().as_deref() == Some(&name)
            })
        })
    })?;
    Some((path, index))
}

#[cfg(windows)]
fn system_font(language: Language) -> Option<(PathBuf, u32)> {
    use font_kit::{
        family_name::FamilyName, handle::Handle, properties::Properties, source::SystemSource,
    };
    let source = SystemSource::new();
    let (postscript, family) = match language {
        Language::ZhHant => ("MicrosoftJhengHeiRegular", "Microsoft JhengHei"),
        Language::Ja => ("YuGothicUI-Regular", "Yu Gothic UI"),
        Language::Ko => ("MalgunGothic", "Malgun Gothic"),
        _ => ("MicrosoftYaHei", "Microsoft YaHei"),
    };
    let handle = source
        .select_by_postscript_name(postscript)
        .or_else(|_| {
            source.select_best_match(&[FamilyName::Title(family.into())], &Properties::new())
        })
        .ok()?;
    if let Handle::Path { path, font_index } = handle {
        Some((path, font_index))
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn system_font(language: Language) -> Option<(PathBuf, u32)> {
    let lang = match language {
        Language::ZhHans => "zh-cn",
        Language::ZhHant => "zh-tw",
        _ => language.code(),
    };
    let result = std::process::Command::new("fc-match")
        .args([
            "-f",
            "%{file}\n%{index}\n",
            &format!("sans-serif:lang={lang}"),
        ])
        .output()
        .ok()?;
    if !result.status.success() {
        return None;
    }
    let output = std::str::from_utf8(&result.stdout).ok()?;
    let mut lines = output.lines();
    let path = PathBuf::from(lines.next()?);
    let index = lines.next()?.parse().ok()?;
    Some((path, index))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "depends on installed macOS fonts"]
    fn pingfang_is_loaded_from_the_system() {
        for (language, expected) in [
            (Language::ZhHans, "PingFangSC-Regular"),
            (Language::ZhHant, "PingFangTC-Regular"),
        ] {
            let (path, index) = system_font(language).expect("system PingFang");
            let data = std::fs::read(path).unwrap();
            let face = ttf_parser::Face::parse(&data, index).unwrap();
            assert!(
                face.names()
                    .into_iter()
                    .any(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME
                        && n.to_string().as_deref() == Some(expected)),
                "wrong face for {language:?}"
            );
        }
    }
}
