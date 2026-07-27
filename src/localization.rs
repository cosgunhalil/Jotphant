//! Localization: translated UI strings loaded from embedded YAML catalogs.
//!
//! Each language is one file under `locales/`, compiled into the binary with
//! `include_str!`. Nested YAML sections flatten to dotted keys (`board.add`), and
//! placeholders use `{curly_braces}`. Lookups fall back to English, and a missing key
//! returns the key itself — translation gaps are visible, never a crash. The
//! [`Localizer`] is constructor-injected like every other collaborator (no globals).

use std::collections::HashMap;

use crate::domain::config::Language;

/// The embedded reference catalog; every other catalog is checked against its keys.
const EN: &str = include_str!("../locales/en.yaml");

/// Returns the embedded catalog source for a language.
fn catalog_source(language: Language) -> &'static str {
    match language {
        Language::English => EN,
        Language::Turkish => include_str!("../locales/tr.yaml"),
        Language::Spanish => include_str!("../locales/es.yaml"),
        Language::Azerbaijani => include_str!("../locales/az.yaml"),
    }
}

/// Translates dotted keys into the configured language.
pub struct Localizer {
    entries: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl Localizer {
    /// Builds a localizer for `language`, with English as the fallback.
    #[must_use]
    pub fn new(language: Language) -> Self {
        let fallback =
            parse_catalog(EN).expect("embedded English catalog is valid (enforced by tests)");
        let entries = if language == Language::English {
            fallback.clone()
        } else {
            parse_catalog(catalog_source(language))
                .expect("embedded catalogs are valid (enforced by tests)")
        };
        Self { entries, fallback }
    }

    /// Looks up `key`, falling back to English, then to the key itself.
    #[must_use]
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map_or(key, String::as_str)
    }

    /// Looks up `key` and replaces each `{name}` placeholder with its value.
    #[must_use]
    pub fn t_args(&self, key: &str, args: &[(&str, String)]) -> String {
        let mut text = self.t(key).to_owned();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

/// Parses a YAML catalog into a flat `dotted.key -> text` map.
fn parse_catalog(yaml: &str) -> Result<HashMap<String, String>, String> {
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(yaml).map_err(|error| error.to_string())?;
    let mut entries = HashMap::new();
    flatten(&value, String::new(), &mut entries)?;
    Ok(entries)
}

fn flatten(
    value: &serde_yaml_ng::Value,
    prefix: String,
    out: &mut HashMap<String, String>,
) -> Result<(), String> {
    match value {
        serde_yaml_ng::Value::Mapping(map) => {
            for (key, child) in map {
                let Some(name) = key.as_str() else {
                    return Err(format!("non-string key under {prefix:?}"));
                };
                let child_prefix = if prefix.is_empty() {
                    name.to_owned()
                } else {
                    format!("{prefix}.{name}")
                };
                flatten(child, child_prefix, out)?;
            }
            Ok(())
        }
        serde_yaml_ng::Value::String(text) => {
            out.insert(prefix, text.clone());
            Ok(())
        }
        other => Err(format!(
            "value at {prefix:?} must be a string, got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The reference key set every catalog must match exactly.
    fn english_keys() -> BTreeSet<String> {
        parse_catalog(EN)
            .expect("english catalog parses")
            .into_keys()
            .collect()
    }

    #[test]
    fn every_language_catalog_has_exactly_the_english_keys() {
        let reference = english_keys();
        assert!(!reference.is_empty());
        for language in Language::ALL {
            let keys: BTreeSet<String> = parse_catalog(catalog_source(language))
                .unwrap_or_else(|error| panic!("{} catalog is invalid: {error}", language.code()))
                .into_keys()
                .collect();
            let missing: Vec<_> = reference.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&reference).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "{} catalog mismatch — missing: {missing:?}, extra: {extra:?}",
                language.code()
            );
        }
    }

    #[test]
    fn nested_sections_flatten_to_dotted_keys() {
        let entries =
            parse_catalog("a:\n  b:\n    c: \"deep\"\n  d: \"shallow\"\n").expect("fixture parses");
        assert_eq!(entries.get("a.b.c").map(String::as_str), Some("deep"));
        assert_eq!(entries.get("a.d").map(String::as_str), Some("shallow"));
    }

    #[test]
    fn non_string_values_are_rejected() {
        assert!(parse_catalog("a: 5\n").is_err());
    }

    #[test]
    fn lookup_finds_translations_and_interpolates() {
        let localizer = Localizer::new(Language::English);
        assert_eq!(localizer.t("board.add"), "Add");
        assert_eq!(
            localizer.t_args(
                "app.bank_balance",
                &[("pomos", "11".to_owned()), ("minutes", "55".to_owned())],
            ),
            "Bank: 11 pomos (≈ 55 min)"
        );
    }

    #[test]
    fn missing_keys_fall_back_to_english_then_to_the_key() {
        let localizer = Localizer {
            entries: HashMap::new(),
            fallback: HashMap::from([("known".to_owned(), "from english".to_owned())]),
        };
        assert_eq!(localizer.t("known"), "from english");
        assert_eq!(localizer.t("totally.unknown"), "totally.unknown");
    }

    #[test]
    fn locale_tags_map_to_supported_languages() {
        assert_eq!(Language::from_locale("en-US"), Some(Language::English));
        assert_eq!(Language::from_locale("en"), Some(Language::English));
        assert_eq!(Language::from_locale("tr-TR"), Some(Language::Turkish));
        assert_eq!(Language::from_locale("es-MX"), Some(Language::Spanish));
        assert_eq!(
            Language::from_locale("az-Latn-AZ"),
            Some(Language::Azerbaijani)
        );
        assert_eq!(Language::from_locale("xx-YY"), None);
    }

    #[test]
    fn non_english_lookups_use_their_catalog() {
        let turkish = Localizer::new(Language::Turkish);
        assert_eq!(turkish.t("board.add"), "Ekle");
        let spanish = Localizer::new(Language::Spanish);
        assert_eq!(spanish.t("board.add"), "Añadir");
        let azerbaijani = Localizer::new(Language::Azerbaijani);
        assert_eq!(azerbaijani.t("board.add"), "Əlavə et");
    }
}
