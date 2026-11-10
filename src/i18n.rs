use std::{cell::Cell, collections::HashMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    En,
    ZhHans,
    ZhHant,
    Ja,
    Fr,
    De,
    Ru,
    Es,
    Pt,
    Ko,
}

impl Language {
    pub const ALL: [Self; 10] = [
        Self::En,
        Self::ZhHans,
        Self::ZhHant,
        Self::Ja,
        Self::Fr,
        Self::De,
        Self::Ru,
        Self::Es,
        Self::Pt,
        Self::Ko,
    ];

    pub fn code(self) -> &'static str {
        [
            "en", "zh-Hans", "zh-Hant", "ja", "fr", "de", "ru", "es", "pt-BR", "ko",
        ][self as usize]
    }

    pub fn name(self) -> &'static str {
        [
            "English",
            "简体中文",
            "繁體中文",
            "日本語",
            "Français",
            "Deutsch",
            "Русский",
            "Español",
            "Português (Brasil)",
            "한국어",
        ][self as usize]
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        let tag = tag.to_ascii_lowercase().replace('_', "-");
        let parts: Vec<_> = tag.split(['-', '.']).collect();
        Some(match *parts.first()? {
            "zh" if parts
                .iter()
                .any(|p| matches!(*p, "hant" | "tw" | "hk" | "mo")) =>
            {
                Self::ZhHant
            }
            "zh" => Self::ZhHans,
            "en" => Self::En,
            "ja" => Self::Ja,
            "fr" => Self::Fr,
            "de" => Self::De,
            "ru" => Self::Ru,
            "es" => Self::Es,
            "pt" => Self::Pt,
            "ko" => Self::Ko,
            _ => return None,
        })
    }

    pub fn system() -> Self {
        sys_locale::get_locales()
            .find_map(|tag| Self::from_tag(&tag))
            .unwrap_or_default()
    }
}

thread_local! {
    static ACTIVE: Cell<Language> = const { Cell::new(Language::En) };
}

pub fn activate(language: Language) {
    ACTIVE.set(language);
}

const CATALOG: &str = include_str!("../locales/ui.tsv");
const DIAGNOSTICS: &str = include_str!("../locales/diagnostics.tsv");
type Row = [&'static str; Language::ALL.len()];
static CATALOGUE: OnceLock<HashMap<&'static str, Row>> = OnceLock::new();

fn rows() -> impl Iterator<Item = &'static str> {
    CATALOG
        .lines()
        .chain(DIAGNOSTICS.lines())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

pub fn t(key: &'static str) -> &'static str {
    let table = CATALOGUE.get_or_init(|| {
        rows()
            .map(|line| {
                let (key, translations) = line.split_once('\t').expect("translation key");
                let row = translations
                    .split('\t')
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("ten translations per key");
                (key, row)
            })
            .collect()
    });
    let row = table.get(key);
    debug_assert!(row.is_some(), "missing translation {key}");
    row.map_or(key, |row| row[ACTIVE.get() as usize])
}

/// Inserts arguments by index so translators can reorder them freely.
pub fn message(key: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = String::new();
    let mut rest = t(key);
    while let Some((before, after)) = rest.split_once('{') {
        result.push_str(before);
        if let Some((index, next)) = after.split_once('}')
            && let Ok(index) = index.parse::<usize>()
            && let Some(arg) = args.get(index)
        {
            use std::fmt::Write as _;
            let _ = write!(result, "{arg}");
            rest = next;
        } else {
            result.push('{');
            rest = after;
        }
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_rows_cover_every_language_and_keep_placeholders() {
        let mut keys = std::collections::HashSet::new();
        for line in rows() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), Language::ALL.len() + 1, "{}", fields[0]);
            assert!(
                fields.iter().all(|field| !field.trim().is_empty()),
                "{}",
                fields[0]
            );
            assert!(keys.insert(fields[0]), "{}", fields[0]);
            for index in 0..5 {
                let marker = format!("{{{index}}}");
                for field in &fields[2..] {
                    assert_eq!(
                        fields[1].matches(&marker).count(),
                        field.matches(&marker).count(),
                        "{} {field}",
                        fields[0]
                    );
                }
            }
        }
    }

    #[test]
    fn locale_detection_distinguishes_chinese_scripts() {
        assert_eq!(Language::from_tag("zh_HK.UTF-8"), Some(Language::ZhHant));
        assert_eq!(Language::from_tag("zh-Hans-SG"), Some(Language::ZhHans));
        assert_eq!(Language::from_tag("de-AT"), Some(Language::De));
        assert_eq!(Language::from_tag("pt-PT"), Some(Language::Pt));
    }
}
