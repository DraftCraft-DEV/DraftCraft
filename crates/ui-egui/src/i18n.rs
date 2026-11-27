//! Display-time localization, following the sibling Craft apps' TSV convention.
//! English strings remain lookup keys; command IDs, drawing data and automation stay unchanged.
//! Add a catalog, a `LANGUAGES` row and a preference variant to introduce another language.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preference {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "el")]
    Greek,
    #[serde(rename = "uk")]
    Ukrainian,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "pt-BR")]
    BrazilianPortuguese,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "cs")]
    Czech,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "zh")]
    Chinese,
    #[serde(rename = "zh-TW")]
    TraditionalChinese,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "sv")]
    Swedish,
    #[serde(rename = "ar")]
    Arabic,
    #[serde(rename = "hu")]
    Hungarian,
    #[serde(rename = "tr")]
    Turkish,
    #[serde(rename = "ka")]
    Georgian,
    #[serde(rename = "bs")]
    Bosnian,
    #[serde(rename = "vi")]
    Vietnamese,
    #[serde(rename = "ta")]
    Tamil,
    #[serde(rename = "af")]
    Afrikaans,
    #[serde(rename = "az")]
    Azerbaijani,
    #[serde(rename = "be")]
    Belarusian,
    #[serde(rename = "bg")]
    Bulgarian,
    #[serde(rename = "ca")]
    Catalan,
    #[serde(rename = "da")]
    Danish,
    #[serde(rename = "et")]
    Estonian,
    #[serde(rename = "fi")]
    Finnish,
    #[serde(rename = "fil")]
    Filipino,
    #[serde(rename = "gl")]
    Galician,
    #[serde(rename = "hr")]
    Croatian,
    #[serde(rename = "id")]
    Indonesian,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "kk")]
    Kazakh,
    #[serde(rename = "ky")]
    Kyrgyz,
    #[serde(rename = "mn")]
    Mongolian,
    #[serde(rename = "ms")]
    Malay,
    #[serde(rename = "nb")]
    NorwegianBokmal,
    #[serde(rename = "nl")]
    Dutch,
    #[serde(rename = "sk")]
    Slovak,
    #[serde(rename = "so")]
    Somali,
    #[serde(rename = "sq")]
    Albanian,
    #[serde(rename = "sr")]
    Serbian,
    #[serde(rename = "sr-Latn")]
    SerbianLatin,
    #[serde(rename = "sw")]
    Swahili,
    #[serde(rename = "tg")]
    Tajik,
    #[serde(rename = "uz")]
    Uzbek,
    #[serde(rename = "zu")]
    Zulu,
}
impl Preference {
    pub const ALL: [Self; 51] = [
        Self::Auto,
        Self::English,
        Self::Greek,
        Self::Ukrainian,
        Self::Polish,
        Self::BrazilianPortuguese,
        Self::German,
        Self::Spanish,
        Self::Czech,
        Self::Russian,
        Self::French,
        Self::Chinese,
        Self::TraditionalChinese,
        Self::Italian,
        Self::Korean,
        Self::Swedish,
        Self::Arabic,
        Self::Hungarian,
        Self::Turkish,
        Self::Georgian,
        Self::Bosnian,
        Self::Vietnamese,
        Self::Tamil,
        Self::Afrikaans,
        Self::Azerbaijani,
        Self::Belarusian,
        Self::Bulgarian,
        Self::Catalan,
        Self::Danish,
        Self::Estonian,
        Self::Finnish,
        Self::Filipino,
        Self::Galician,
        Self::Croatian,
        Self::Indonesian,
        Self::Japanese,
        Self::Kazakh,
        Self::Kyrgyz,
        Self::Mongolian,
        Self::Malay,
        Self::NorwegianBokmal,
        Self::Dutch,
        Self::Slovak,
        Self::Somali,
        Self::Albanian,
        Self::Serbian,
        Self::SerbianLatin,
        Self::Swahili,
        Self::Tajik,
        Self::Uzbek,
        Self::Zulu,
    ];

    /// Every preference in the order the Interface Language dialog lists them: Auto, then English
    /// (the source language), then the rest by the language's own name with accents folded, so
    /// Čeština sits under C and Magyar under M. Names in other scripts follow the Latin ones in
    /// code point order: Greek, Cyrillic, Arabic, Tamil, Georgian, then the CJK languages. The
    /// order follows the registry's native name even where the dialog shows an English label
    /// instead (Korean without a Hangul face), so the list does not reorder when a font appears.
    pub fn for_picker() -> Vec<Self> {
        let mut rest: Vec<Self> = Self::ALL.into_iter().filter(|p| !matches!(p, Self::Auto | Self::English)).collect();
        rest.sort_by_cached_key(|p| name_sort_key(p.native_name()));
        [Self::Auto, Self::English].into_iter().chain(rest).collect()
    }

    /// The language's own name from the registry, whatever the dialog currently shows for it.
    fn native_name(self) -> &'static str {
        LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(self.code())).map_or_else(|| self.name(), |l| l.name)
    }

    /// The codes a preference can be given as, for messages ("auto, en or uk").
    pub fn codes() -> String {
        let codes: Vec<&str> = Self::ALL.iter().map(|p| p.code()).collect();
        match codes.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
            _ => codes.join(""),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::English => "en",
            Self::Greek => "el",
            Self::Ukrainian => "uk",
            Self::Polish => "pl",
            Self::BrazilianPortuguese => "pt-BR",
            Self::German => "de",
            Self::Spanish => "es",
            Self::Czech => "cs",
            Self::Russian => "ru",
            Self::French => "fr",
            Self::Chinese => "zh",
            Self::TraditionalChinese => "zh-TW",
            Self::Italian => "it",
            Self::Korean => "ko",
            Self::Swedish => "sv",
            Self::Arabic => "ar",
            Self::Hungarian => "hu",
            Self::Turkish => "tr",
            Self::Georgian => "ka",
            Self::Bosnian => "bs",
            Self::Vietnamese => "vi",
            Self::Tamil => "ta",
            Self::Afrikaans => "af",
            Self::Azerbaijani => "az",
            Self::Belarusian => "be",
            Self::Bulgarian => "bg",
            Self::Catalan => "ca",
            Self::Danish => "da",
            Self::Estonian => "et",
            Self::Finnish => "fi",
            Self::Filipino => "fil",
            Self::Galician => "gl",
            Self::Croatian => "hr",
            Self::Indonesian => "id",
            Self::Japanese => "ja",
            Self::Kazakh => "kk",
            Self::Kyrgyz => "ky",
            Self::Mongolian => "mn",
            Self::Malay => "ms",
            Self::NorwegianBokmal => "nb",
            Self::Dutch => "nl",
            Self::Slovak => "sk",
            Self::Somali => "so",
            Self::Albanian => "sq",
            Self::Serbian => "sr",
            Self::SerbianLatin => "sr-Latn",
            Self::Swahili => "sw",
            Self::Tajik => "tg",
            Self::Uzbek => "uz",
            Self::Zulu => "zu",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => t("Auto (system language)"),
            Self::English => "English",
            Self::Greek => "Ελληνικά",
            Self::Ukrainian => "Українська",
            Self::Polish => "Polski",
            Self::BrazilianPortuguese => "Português (Brasil)",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::Czech => "Čeština",
            Self::Russian => "Русский",
            Self::French => "Français",
            Self::Chinese => "简体中文",
            Self::TraditionalChinese => "繁體中文",
            Self::Italian => "Italiano",
            Self::Korean => {
                if crate::theme::shows_hangul(CURRENT.with(Cell::get)) {
                    "한국어"
                } else {
                    "Korean"
                }
            }
            Self::Swedish => "Svenska",
            Self::Arabic => "العربية",
            Self::Hungarian => "Magyar",
            Self::Turkish => "Türkçe",
            // Georgian letters come from the craft-fonts Georgian face; a build without it shows
            // the English name (as Korean does) and keeps the interface in English.
            Self::Georgian => {
                if crate::theme::can_show("ka") {
                    "ქართული"
                } else {
                    "Georgian"
                }
            }
            Self::Bosnian => "Bosanski",
            // Its letters need a font egui doesn't bundle ([`crate::theme::vietnamese_font`]).
            Self::Vietnamese => {
                if crate::theme::shows_vietnamese(CURRENT.with(Cell::get)) {
                    "Tiếng Việt"
                } else {
                    "Vietnamese"
                }
            }
            // Its letters need a Tamil font, loaded only in the Tamil interface
            // ([`crate::theme::shows_tamil`]); elsewhere the picker reads the English name.
            Self::Tamil => {
                if crate::theme::shows_tamil(CURRENT.with(Cell::get)) {
                    "தமிழ்"
                } else {
                    "Tamil"
                }
            }
            Self::Afrikaans => "Afrikaans",
            // ə needs a system interface font that has it ([`crate::theme::azerbaijani_font`]).
            Self::Azerbaijani => {
                if crate::theme::shows_azerbaijani(CURRENT.with(Cell::get)) {
                    "Azərbaycan dili"
                } else {
                    "Azerbaijani"
                }
            }
            Self::Belarusian => "Беларуская",
            Self::Bulgarian => "Български",
            Self::Catalan => "Català",
            Self::Danish => "Dansk",
            Self::Estonian => "Eesti",
            Self::Finnish => "Suomi",
            Self::Filipino => "Filipino",
            Self::Galician => "Galego",
            Self::Croatian => "Hrvatski",
            Self::Indonesian => "Bahasa Indonesia",
            Self::Kazakh => "Қазақ тілі",
            Self::Kyrgyz => "Кыргызча",
            Self::Mongolian => "Монгол",
            Self::Malay => "Bahasa Melayu",
            Self::NorwegianBokmal => "Norsk bokmål",
            Self::Dutch => "Nederlands",
            Self::Slovak => "Slovenčina",
            Self::Somali => "Soomaali",
            Self::Albanian => "Shqip",
            Self::Serbian => "Српски (ћирилица)",
            Self::SerbianLatin => "Srpski (latinica)",
            Self::Swahili => "Kiswahili",
            Self::Tajik => "Тоҷикӣ",
            Self::Uzbek => "O‘zbekcha",
            Self::Zulu => "isiZulu",
            // Its letters need a Japanese or CJK font; without one the picker reads the English name.
            Self::Japanese => {
                if crate::theme::can_show("ja") {
                    "日本語"
                } else {
                    "Japanese"
                }
            }
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.code() == code)
    }

    /// An invalid or obsolete saved choice follows the system language.
    pub fn restore(saved: Option<&str>) -> Self {
        saved.and_then(Self::parse).unwrap_or_default()
    }

    pub fn resolve(self, locales: &[String]) -> &'static str {
        match self {
            Self::English => "en",
            Self::Greek => "el",
            Self::Ukrainian => "uk",
            Self::Polish => "pl",
            Self::BrazilianPortuguese => "pt-BR",
            Self::German => "de",
            Self::Spanish => "es",
            Self::Czech => "cs",
            Self::Russian => "ru",
            Self::French => "fr",
            Self::Chinese => "zh",
            Self::TraditionalChinese => "zh-TW",
            Self::Italian => "it",
            Self::Korean => "ko",
            Self::Swedish => "sv",
            Self::Arabic => "ar",
            Self::Hungarian => "hu",
            Self::Turkish => "tr",
            Self::Georgian => "ka",
            Self::Bosnian => "bs",
            Self::Vietnamese => "vi",
            Self::Tamil => "ta",
            Self::Afrikaans => "af",
            Self::Azerbaijani => "az",
            Self::Belarusian => "be",
            Self::Bulgarian => "bg",
            Self::Catalan => "ca",
            Self::Danish => "da",
            Self::Estonian => "et",
            Self::Finnish => "fi",
            Self::Filipino => "fil",
            Self::Galician => "gl",
            Self::Croatian => "hr",
            Self::Indonesian => "id",
            Self::Japanese => "ja",
            Self::Kazakh => "kk",
            Self::Kyrgyz => "ky",
            Self::Mongolian => "mn",
            Self::Malay => "ms",
            Self::NorwegianBokmal => "nb",
            Self::Dutch => "nl",
            Self::Slovak => "sk",
            Self::Somali => "so",
            Self::Albanian => "sq",
            Self::Serbian => "sr",
            Self::SerbianLatin => "sr-Latn",
            Self::Swahili => "sw",
            Self::Tajik => "tg",
            Self::Uzbek => "uz",
            Self::Zulu => "zu",
            Self::Auto => locales.iter().find_map(|s| locale_language(s)).unwrap_or("en"),
        }
    }
}

pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    pub source: &'static str,
    /// Index of the `@plural` form for an integer count.
    pub plural: fn(u64) -> usize,
    /// Number of `|`-separated forms every `@plural` row must carry.
    pub plural_forms: usize,
    catalog: OnceLock<Catalog<'static>>,
}

pub static LANGUAGES: [Language; 50] = [
    Language { code: "en", name: "English", source: "", plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    Language { code: "pl", name: "Polski", source: include_str!("i18n/pl.tsv"), plural: plural_pl, plural_forms: 3, catalog: OnceLock::new() },
    // Greek: one for exactly 1; other for zero and all other integer counts.
    Language {
        code: "el", name: "Ελληνικά", source: include_str!("i18n/el.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new()
    },
    Language {
        code: "uk", name: "Українська", source: include_str!("i18n/uk.tsv"), plural: plural_uk, plural_forms: 3, catalog: OnceLock::new()
    },
    Language {
        code: "pt-BR",
        name: "Português (Brasil)",
        source: include_str!("i18n/pt_br.tsv"),
        plural: plural_pt,
        plural_forms: 2,
        catalog: OnceLock::new(),
    },
    // German plurals follow the English rule: one (1), other.
    Language { code: "de", name: "Deutsch", source: include_str!("i18n/de.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Spanish integer plurals follow the English rule too.
    Language { code: "es", name: "Español", source: include_str!("i18n/es.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    Language { code: "cs", name: "Čeština", source: include_str!("i18n/cs.tsv"), plural: plural_cs, plural_forms: 3, catalog: OnceLock::new() },
    // Russian integer plurals follow the Ukrainian rule: one (1, 21), few (2–4), many.
    Language {
        code: "ru", name: "Русский", source: include_str!("i18n/ru.tsv"), plural: plural_uk, plural_forms: 3, catalog: OnceLock::new()
    },
    // French integer plurals (CLDR `fr`): one (0, 1), other, the Portuguese rule.
    Language { code: "fr", name: "Français", source: include_str!("i18n/fr.tsv"), plural: plural_pt, plural_forms: 2, catalog: OnceLock::new() },
    // Chinese has no grammatical plural: both `@plural` forms read the same. Its glyphs come from
    // an installed CJK font (`theme::cjk_font`); without one the interface stays English.
    Language { code: "zh", name: "简体中文", source: include_str!("i18n/zh.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Traditional Chinese, as written in Taiwan (#33): the same font needs and plural forms.
    Language {
        code: "zh-TW",
        name: "繁體中文",
        source: include_str!("i18n/zh_tw.tsv"),
        plural: plural_en,
        plural_forms: 2,
        catalog: OnceLock::new(),
    },
    // Italian integer plurals (CLDR `it`) follow the English rule: one (1), other.
    Language { code: "it", name: "Italiano", source: include_str!("i18n/it.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Korean has one integer plural form and needs a Hangul-capable interface font.
    Language { code: "ko", name: "한국어", source: include_str!("i18n/ko.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new() },
    // Swedish plurals follow the English rule: one (1), other.
    Language { code: "sv", name: "Svenska", source: include_str!("i18n/sv.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // العربية (Arabic) is right-to-left; an installed Arabic font (`theme::arabic_font`)
    // covers its glyphs, and the shell keeps egui's left-to-right run order. Six plural
    // forms: zero, one, two, few, many, other.
    Language {
        code: "ar", name: "العربية", source: include_str!("i18n/ar.tsv"), plural: plural_arabic, plural_forms: 6, catalog: OnceLock::new()
    },
    // Hungarian uses the singular after a numeral, so one form covers every count (the Korean rule).
    Language { code: "hu", name: "Magyar", source: include_str!("i18n/hu.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new() },
    // Turkish keeps a noun singular after a number ("3 commit"), so like Korean it has one form.
    Language { code: "tr", name: "Türkçe", source: include_str!("i18n/tr.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new() },
    // Georgian keeps a noun singular after a numeral ("5 კომიტი"), so one form, the Korean rule.
    // Its letters come from the craft-fonts `Geor` face (`theme::can_show`).
    Language {
        code: "ka", name: "ქართული", source: include_str!("i18n/ka.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new()
    },
    // Bosnian (Latin script, Ijekavian): one (1, 21), few (2–4, 22–24, not 12–14), other — the
    // Ukrainian rule for integer counts.
    Language { code: "bs", name: "Bosanski", source: include_str!("i18n/bs.tsv"), plural: plural_uk, plural_forms: 3, catalog: OnceLock::new() },
    // Vietnamese doesn't inflect a noun for number ("3 commit"), so like Korean it has one form.
    Language {
        code: "vi", name: "Tiếng Việt", source: include_str!("i18n/vi.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new()
    },
    // தமிழ் (Tamil) plurals follow the English rule: one (1), other. Its letters come from an
    // installed Tamil font or the craft-fonts `Taml` face (`theme::can_show`).
    Language {
        code: "ta", name: "தமிழ்", source: include_str!("i18n/ta.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new()
    },
    // Afrikaans plurals follow the English rule: one (1), other.
    Language { code: "af", name: "Afrikaans", source: include_str!("i18n/af.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Azerbaijani (Latin script): one (1), other, the English rule. Its ə needs a system
    // interface font that has it (`theme::can_show`).
    Language {
        code: "az",
        name: "Azərbaycan dili",
        source: include_str!("i18n/az.tsv"),
        plural: plural_en,
        plural_forms: 2,
        catalog: OnceLock::new(),
    },
    // Belarusian integer plurals follow the Ukrainian rule: one (1, 21), few (2–4), many.
    Language {
        code: "be", name: "Беларуская", source: include_str!("i18n/be.tsv"), plural: plural_uk, plural_forms: 3, catalog: OnceLock::new()
    },
    // Bulgarian plurals follow the English rule: one (1), other.
    Language {
        code: "bg", name: "Български", source: include_str!("i18n/bg.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new()
    },
    // Catalan integer plurals follow the English rule: one (1), other.
    Language { code: "ca", name: "Català", source: include_str!("i18n/ca.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Danish integer plurals follow the English rule: one (1), other.
    Language { code: "da", name: "Dansk", source: include_str!("i18n/da.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Estonian plurals follow the English rule: one (1), other.
    Language { code: "et", name: "Eesti", source: include_str!("i18n/et.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Finnish plurals follow the English rule: one (1), other.
    Language { code: "fi", name: "Suomi", source: include_str!("i18n/fi.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Filipino (CLDR `fil`): one for 1, 2, 3 and counts not ending in 4, 6 or 9; other for the rest.
    Language { code: "fil", name: "Filipino", source: include_str!("i18n/fil.tsv"), plural: plural_fil, plural_forms: 2, catalog: OnceLock::new() },
    // Galician integer plurals follow the English rule: one (1), other.
    Language { code: "gl", name: "Galego", source: include_str!("i18n/gl.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Croatian: one (1, 21), few (2–4, 22–24, not 12–14), other — the Ukrainian rule for integers.
    Language { code: "hr", name: "Hrvatski", source: include_str!("i18n/hr.tsv"), plural: plural_uk, plural_forms: 3, catalog: OnceLock::new() },
    // Indonesian doesn't inflect a noun for number, so like Korean it has one form.
    Language {
        code: "id",
        name: "Bahasa Indonesia",
        source: include_str!("i18n/id.tsv"),
        plural: plural_ko,
        plural_forms: 1,
        catalog: OnceLock::new(),
    },
    // Japanese has no grammatical plural, so one form. Its glyphs come from an installed Japanese or CJK
    // font, or a craft-fonts `Jpan` face (`theme::can_show`); without one the interface stays English.
    Language { code: "ja", name: "日本語", source: include_str!("i18n/ja.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new() },
    // Kazakh (Cyrillic): one (1), other, the English rule.
    Language {
        code: "kk", name: "Қазақ тілі", source: include_str!("i18n/kk.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new()
    },
    // Kyrgyz (Cyrillic): one (1), other, the English rule.
    Language {
        code: "ky", name: "Кыргызча", source: include_str!("i18n/ky.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new()
    },
    // Mongolian (Cyrillic): one (1), other, the English rule.
    Language { code: "mn", name: "Монгол", source: include_str!("i18n/mn.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Malay doesn't inflect a noun for number, so like Korean it has one form.
    Language { code: "ms", name: "Bahasa Melayu", source: include_str!("i18n/ms.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new() },
    // Norwegian Bokmål plurals follow the English rule: one (1), other. `no` and `nn` tags select it.
    Language { code: "nb", name: "Norsk bokmål", source: include_str!("i18n/nb.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Dutch plurals follow the English rule: one (1), other.
    Language { code: "nl", name: "Nederlands", source: include_str!("i18n/nl.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Slovak integer plurals follow the Czech rule: one (1), few (2–4), other.
    Language { code: "sk", name: "Slovenčina", source: include_str!("i18n/sk.tsv"), plural: plural_cs, plural_forms: 3, catalog: OnceLock::new() },
    // Somali plurals follow the English rule: one (1), other.
    Language { code: "so", name: "Soomaali", source: include_str!("i18n/so.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Albanian plurals follow the English rule: one (1), other.
    Language { code: "sq", name: "Shqip", source: include_str!("i18n/sq.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Serbian in Cyrillic, the default for `sr` tags: one (1, 21), few (2–4), other — the Ukrainian rule.
    Language {
        code: "sr",
        name: "Српски (ћирилица)",
        source: include_str!("i18n/sr.tsv"),
        plural: plural_uk,
        plural_forms: 3,
        catalog: OnceLock::new(),
    },
    // Serbian in Latin script, for `sr-Latn` tags: the same plural rule as Cyrillic Serbian.
    Language {
        code: "sr-Latn",
        name: "Srpski (latinica)",
        source: include_str!("i18n/sr_latn.tsv"),
        plural: plural_uk,
        plural_forms: 3,
        catalog: OnceLock::new(),
    },
    // Swahili plurals follow the English rule: one (1), other.
    Language { code: "sw", name: "Kiswahili", source: include_str!("i18n/sw.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Tajik keeps a noun singular after a numeral, so like Korean it has one form.
    Language { code: "tg", name: "Тоҷикӣ", source: include_str!("i18n/tg.tsv"), plural: plural_ko, plural_forms: 1, catalog: OnceLock::new() },
    // Uzbek (Latin script): one (1), other, the English rule.
    Language { code: "uz", name: "O‘zbekcha", source: include_str!("i18n/uz.tsv"), plural: plural_en, plural_forms: 2, catalog: OnceLock::new() },
    // Zulu (CLDR `zu`): one (0, 1), other, the Portuguese rule.
    Language { code: "zu", name: "isiZulu", source: include_str!("i18n/zu.tsv"), plural: plural_pt, plural_forms: 2, catalog: OnceLock::new() },
];

/// BCP 47 and POSIX locale tags, including `uk-UA`, `uk_UA.UTF-8`, `pt_BR.UTF-8` and `sr_RS@latin`.
/// A catalog is chosen by the tag's primary language, so any `pt-*` tag selects `pt-BR`; Chinese
/// also by its script or region: Traditional (`zh-TW`) for `Hant`, Taiwan, Hong Kong and Macau,
/// else Simplified (`zh`).
pub fn locale_language(tag: &str) -> Option<&'static str> {
    // The language, script and region subtags: a POSIX codeset (`.UTF-8`) or modifier (`@…`) ends them.
    let subtags = tag.trim().split(['.', '@']).next().unwrap_or_default();
    let primary = tag.trim().split(['-', '_', '.', '@']).next()?;
    let has = |names: &[&str]| subtags.split(['-', '_']).skip(1).any(|s| names.iter().any(|n| s.eq_ignore_ascii_case(n)));
    // Serbian: Latin script for `Latn` or `@latin`, else Cyrillic. Norwegian `no` and Nynorsk `nn` read
    // Bokmål; the older `tl` (Tagalog) reads Filipino and `in` Indonesian.
    if primary.eq_ignore_ascii_case("sr") {
        // POSIX writes the script as a modifier: `sr_RS@latin`.
        let latin = has(&["Latn"]) || tag.to_ascii_lowercase().contains("@latin");
        return Some(if latin { "sr-Latn" } else { "sr" });
    }
    let primary = match primary.to_ascii_lowercase().as_str() {
        "no" | "nn" => "nb",
        "tl" => "fil",
        "in" => "id",
        _ => primary,
    };
    if primary.eq_ignore_ascii_case("zh") {
        return Some(if !has(&["Hans"]) && has(&["Hant", "TW", "HK", "MO"]) { "zh-TW" } else { "zh" });
    }
    LANGUAGES.iter().find(|l| l.code.split('-').next().is_some_and(|p| p.eq_ignore_ascii_case(primary))).map(|l| l.code)
}

thread_local! {
    // Scoped to a single app's render. Headless calls and parallel tests default to English.
    static CURRENT: Cell<&'static str> = const { Cell::new("en") };
}

pub fn with_language<R>(language: &'static str, draw: impl FnOnce() -> R) -> R {
    struct Restore(&'static str);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.set(self.0);
        }
    }
    let _restore = Restore(CURRENT.replace(language));
    draw()
}

type Catalog<'a> = HashMap<(&'a str, &'a str), &'a str>;

/// Malformed rows are ignored at runtime and diagnosed by the catalog tests.
fn parse(source: &str) -> (Catalog<'_>, Vec<String>) {
    let mut entries = HashMap::new();
    let mut errors = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<_> = line.split('\t').collect();
        match columns.as_slice() {
            [context, key, value] if !key.is_empty() && !value.is_empty() => {
                if entries.insert((*context, *key), *value).is_some() {
                    errors.push(format!("line {}: duplicate key {key}", i + 1));
                }
            }
            _ => errors.push(format!("line {}: expected context, source, translation", i + 1)),
        }
    }
    (entries, errors)
}

fn catalog(language: &str) -> Option<&'static Catalog<'static>> {
    LANGUAGES.iter().find(|l| l.code == language && !l.source.is_empty()).map(|l| l.catalog.get_or_init(|| parse(l.source).0))
}

pub fn tr<'a>(language: &str, source: &'a str) -> &'a str {
    catalog(language).and_then(|c| c.get(&("", source))).copied().unwrap_or(source)
}

pub fn t(source: &str) -> &str {
    CURRENT.with(|l| tr(l.get(), source))
}

/// Menu headings can differ from command or object labels (e.g. Czech Kótování / Kóta).
/// Catalogs without a `menu` entry retain their ordinary translation.
pub fn tr_menu<'a>(language: &str, source: &'a str) -> &'a str {
    catalog(language).and_then(|c| c.get(&("menu", source))).copied().unwrap_or_else(|| tr(language, source))
}

pub fn menu(source: &str) -> &str {
    CURRENT.with(|l| tr_menu(l.get(), source))
}

/// Named placeholders allow translations to reorder values. Inserted values are never looked up
/// or substituted again, so a user's name containing `{n}` is preserved verbatim.
pub fn format(source: &str, values: &[(&str, String)]) -> String {
    let text = t(source);
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, tail)) = rest.split_once('{') {
        result.push_str(before);
        let Some((key, after)) = tail.split_once('}') else {
            result.push('{');
            rest = tail;
            break;
        };
        if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            result.push_str(value);
        } else {
            result.push('{');
            result.push_str(key);
            result.push('}');
        }
        rest = after;
    }
    result.push_str(rest);
    result
}

/// Polish plurals: one (1), few (2–4, 22–24), many (0, 5–21, 25–31).
pub fn plural_pl(n: u64) -> usize {
    if n == 1 {
        0
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        1
    } else {
        2
    }
}

/// Ukrainian integer plurals: one (1, 21), few (2–4, 22–24), many (0, 5–20, 25…).
pub fn plural_uk(n: u64) -> usize {
    match (n % 10, n % 100) {
        (1, 11) | (2..=4, 12..=14) => 2,
        (1, _) => 0,
        (2..=4, _) => 1,
        _ => 2,
    }
}

/// Portuguese integer plurals (CLDR `pt`): one (0, 1), other (2 and up).
pub fn plural_pt(n: u64) -> usize {
    usize::from(n > 1)
}

/// Korean uses the same count form for every integer.
fn plural_ko(_n: u64) -> usize {
    0
}

/// Arabic integer plurals (CLDR `ar`): zero (0), one (1), two (2), few (3–10, 103–110),
/// many (11–99) — the same rule pdfcraft uses, so the sibling apps agree.
pub fn plural_arabic(n: u64) -> usize {
    match (n, n % 100) {
        (0, _) => 0,
        (1, _) => 1,
        (2, _) => 2,
        (_, 3..=10) => 3,
        (_, 11..=99) => 4,
        _ => 5,
    }
}

/// Filipino (CLDR `fil`): one for 1, 2 and 3 and for any count whose last digit is not 4, 6 or 9.
pub fn plural_fil(n: u64) -> usize {
    usize::from(n > 3 && matches!(n % 10, 4 | 6 | 9))
}

fn plural_en(n: u64) -> usize {
    usize::from(n != 1)
}

/// Czech integer plurals: one (1), few (2–4), other (0 and 5 or more).
pub fn plural_cs(n: u64) -> usize {
    match n {
        1 => 0,
        2..=4 => 1,
        _ => 2,
    }
}

pub fn count(n: u64, one: &str, other: &str) -> String {
    let language = CURRENT.get();
    let source = catalog(language).and_then(|c| c.get(&("@plural", one))).copied();
    let plural = LANGUAGES.iter().find(|l| l.code == language).map_or(plural_en as fn(u64) -> usize, |l| l.plural);
    let form = source.and_then(|s| s.split('|').nth(plural(n))).unwrap_or(if n == 1 { one } else { other });
    format(form, &[("n", n.to_string())])
}

#[macro_export]
macro_rules! tl {
    ($source:literal) => {
        $crate::i18n::t($source)
    };
}

#[macro_export]
macro_rules! tf {
    ($source:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::format($source, &[$((stringify!($name), ($value).to_string())),*])
    };
}

#[cfg(test)]
mod tests;

/// `name` lowercased with the Latin accents that occur in language names folded away, so a
/// byte-wise sort reads alphabetically. Other scripts are left as they are.
fn name_sort_key(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è'..='ë' | 'ē' | 'ě' | 'ę' | 'ế' | 'ệ' => 'e',
            'ì'..='ï' | 'ī' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ő' => 'o',
            'ś' | 'š' => 's',
            'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod picker_order_tests {
    use super::*;

    #[test]
    fn picker_lists_auto_and_english_first_then_names_alphabetically() {
        let order = Preference::for_picker();
        assert_eq!(&order[..2], &[Preference::Auto, Preference::English]);
        assert_eq!(order.len(), Preference::ALL.len());
        let keys: Vec<String> = order[2..].iter().map(|p| name_sort_key(p.native_name())).collect();
        assert!(keys.windows(2).all(|w| w[0] <= w[1]), "{keys:?}");
        let pos = |p: Preference| order.iter().position(|q| *q == p).unwrap_or(usize::MAX);
        assert!(pos(Preference::Czech) < pos(Preference::German), "Čeština sorts under C");
        assert!(pos(Preference::Italian) < pos(Preference::Hungarian), "Magyar sorts under M");
        assert!(pos(Preference::Turkish) < pos(Preference::Greek), "Latin names come before Greek");
        assert!(pos(Preference::Bosnian) < pos(Preference::Czech), "Bosanski sorts under B");
        assert!(pos(Preference::Swedish) < pos(Preference::Vietnamese), "Tiếng Việt sorts under T");
        assert!(pos(Preference::Vietnamese) < pos(Preference::Turkish), "Tiếng comes before Türkçe");
        assert!(pos(Preference::Arabic) < pos(Preference::Tamil) && pos(Preference::Tamil) < pos(Preference::Georgian), "தமிழ் sorts by code point");
        assert_eq!(name_sort_key("Türkçe"), "turkce");
    }
}
