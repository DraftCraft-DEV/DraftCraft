# Interface language roadmap

Written 2026-11-26. The aim: every language ArtCraft's CADCraft covers (done) and a route to every language
CadCraft Studio covers. What is measured is marked as measured; the rest is marked as my reading.

## Where we are

| | Languages besides English | Source |
|---|---|---|
| DraftCraft | **21**: ar bs cs de el es fr hu it ka ko pl pt-BR ru sv ta tr uk vi zh zh-TW | `crates/ui-egui/src/i18n.rs` |
| ArtCraft's CADCraft | 20: ours without zh-TW. Polish, Georgian and Tamil were the three we lacked; they are in now. | upstream `cadcraft/main` at `a3ed0682` |
| CadCraft Studio, the launcher (`hub/lang`) | **75** (76 files with `en`), 762 strings each: the launcher's own words, not CAD commands | `getcadcraft/studio` 0.11.0, `8fd681c` (2026-11-23) |
| CadCraft Studio, the CAD apps (`language-packs`) | **54**, about 44,700 entries each across all apps | `getcadcraft/language-packs`, pack version 2026.11.01 |

You said 77. I count 75 plus English (76 files) in the repository, and 54 in the packs; getcadcraft.com, where the apps fetch the live
packs, is not reachable from this environment, so I could not see whether two more were added since. Tell me which and I will add them.

All 21 of our languages are in Studio's launcher set (our one Spanish stands for both of its Spanish variants, so 22 of its 75 are covered); 53 are not ours yet.
Every Studio translation is a machine draft that no native speaker has reviewed; its own files say so. The licence is Apache-2.0.

## What Studio's packs can seed (measured)

Each pack entry stores the translation and a 12-character hash of the English sentence it translates. Hashing our English strings the same way
(`sha256`, first 12 hex characters) matches **485 of our 1,673 strings (29%)** in every one of the 54 pack languages: the shared words (Cancel,
Layers, Settings...). The other 71% are AutoCAD-style command names and prompts that Studio does not have, so they need new translations.

The 21 launcher-only languages have no pack: only 20 of our strings (About, Add, Cancel, Close, Copy...) match their launcher text. They start from nothing.

Whether the drafts are good enough to ship: on the strings both sides have, Studio's draft equals the catalog we already have for the same
language only **42% to 82%** of the time (Portuguese 82%, Italian 74%, Swedish 72%, Russian 65%, Arabic 43%, Greek 42%), so a seed is a
starting point that a native speaker must review, not a finished translation.

## Languages

Column key. **Font**: `egui` = egui's bundled face covers every letter Studio's text uses in that language; `shipped` = a face in the `craft-fonts`
revision our releases pin (`b076417b`); `pin bump` = a face exists in craft-fonts `83c36c8` or later, so raising `CRAFT_FONTS_REF` fixes it
(+4.3 MB on desktop for the eight new Indic faces); `none` = no `craft-fonts` face covers the script. Checked by testing each language's letters against the faces
themselves, on Studio's launcher text plus its CAD-app strings where a pack exists (a sample, not our full UI).
**Plurals** use the rules we already have, my reading of CLDR; verify before implementing. **Wave**: 1 = can go on today's code; 2 = one small
addition first (a plural rule, a pin bump, or a right-to-left or complex-script rendering check); 3 = needs a font we do not have; V = variant of a language we have.

| Code | Language | Script | Ours | ArtCraft | Studio packs | Font | Plurals | Wave |
|---|---|---|:-:|:-:|:-:|---|---|:-:|
| `ar` | Arabic · العربية | Arab RTL | ✓ | ✓ | ✓ | shipped | 6 (ar rule) | — |
| `bs` | Bosnian · Bosanski | Latn | ✓ | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | — |
| `cs` | Czech · Čeština | Latn | ✓ | ✓ | ✓ | egui | 3 (cs rule) | — |
| `de` | German · Deutsch | Latn | ✓ | ✓ | ✓ | egui | 2 (one = 1) | — |
| `el` | Greek · Ελληνικά | Grek | ✓ | ✓ | ✓ | egui | 2 (one = 1) | — |
| `es-ES` | Spanish (Spain) · Español (España) | Latn | ✓ | ✓ | ✓ | egui | 2 (one = 1) | — |
| `es-MX` | Spanish (Mexico) · Español (México) | Latn | ✓ | ✓ | ✓ | egui | 2 (one = 1) | — |
| `fr` | French · Français | Latn | ✓ | ✓ | ✓ | egui | 2 (0 and 1) | — |
| `hu` | Hungarian · Magyar | Latn | ✓ | ✓ | ✓ | egui | 1 form | — |
| `it` | Italian · Italiano | Latn | ✓ | ✓ | ✓ | egui | 2 (one = 1) | — |
| `ka` | Georgian · ქართული | Geor | ✓ | ✓ | – | shipped | 1 form | — |
| `ko` | Korean · 한국어 | Kore | ✓ | ✓ | ✓ | shipped | 1 form | — |
| `pl` | Polish · Polski | Latn | ✓ | ✓ | ✓ | egui | 3 (pl rule) | — |
| `pt-BR` | Portuguese (Brazil) · Português (Brasil) | Latn | ✓ | ✓ | ✓ | egui | 2 (0 and 1) | — |
| `ru` | Russian · Русский | Cyrl | ✓ | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | — |
| `sv` | Swedish · Svenska | Latn | ✓ | ✓ | ✓ | egui | 2 (one = 1) | — |
| `ta` | Tamil · தமிழ் | Taml | ✓ | ✓ | ✓ | pin bump | 2 (one = 1) | — |
| `tr` | Turkish · Türkçe | Latn | ✓ | ✓ | ✓ | egui | 1 form | — |
| `uk` | Ukrainian · Українська | Cyrl | ✓ | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | — |
| `vi` | Vietnamese · Tiếng Việt | Latn | ✓ | ✓ | ✓ | shipped | 1 form | — |
| `zh-Hans` | Chinese (Simplified) · 简体中文 | Hans | ✓ | ✓ | ✓ | shipped | 1 form | — |
| `zh-Hant` | Chinese (Traditional) · 繁體中文 | Hant | ✓ | – | ✓ | shipped | 1 form | — |
| `af` | Afrikaans · Afrikaans | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `az` | Azerbaijani · Azərbaycanca | Latn | – | – | – | shipped | 2 (one = 1) | 1 |
| `be` | Belarusian · Беларуская | Cyrl | – | – | – | egui | 3 (1 · 2-4 · rest) | 1 |
| `bg` | Bulgarian · Български | Cyrl | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `ca` | Catalan · Català | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `da` | Danish · Dansk | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `et` | Estonian · Eesti | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `fi` | Finnish · Suomi | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `fil` | Filipino · Filipino | Latn | – | – | ✓ | egui | 2, verify | 1 |
| `gl` | Galician · Galician | Latn | – | – | – | egui | 2 (one = 1) | 1 |
| `hr` | Croatian · Hrvatski | Latn | – | – | ✓ | egui | 3 (1 · 2-4 · rest) | 1 |
| `id` | Indonesian · Bahasa Indonesia | Latn | – | – | ✓ | egui | 1 form | 1 |
| `ja` | Japanese · 日本語 | Jpan | – | – | ✓ | shipped | 1 form | 1 |
| `kk` | Kazakh · Қазақша | Cyrl | – | – | – | egui | 2 (one = 1) | 1 |
| `ky` | Kyrgyz · Кыргызча | Cyrl | – | – | – | egui | 2 (one = 1) | 1 |
| `mn` | Mongolian · Монгол | Cyrl | – | – | – | egui | 2 (one = 1) | 1 |
| `ms` | Malay · Bahasa Melayu | Latn | – | – | ✓ | egui | 1 form | 1 |
| `nb` | Norwegian Bokmål · Norsk bokmål | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `nl` | Dutch · Nederlands | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `sk` | Slovak · Slovenčina | Latn | – | – | ✓ | egui | 3 (cs rule) | 1 |
| `so` | Somali · Soomaali | Latn | – | – | – | egui | 2 (one = 1) | 1 |
| `sq` | Albanian · Shqip | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `sr-Cyrl` | Serbian (Cyrillic) · Српски (ћирилица) | Cyrl | – | – | ✓ | egui | 3 (1 · 2-4 · rest) | 1 |
| `sr-Latn` | Serbian (Latin) · Srpski (latinica) | Latn | – | – | ✓ | egui | 3 (1 · 2-4 · rest) | 1 |
| `sw` | Swahili · Kiswahili | Latn | – | – | ✓ | egui | 2 (one = 1) | 1 |
| `tg` | Tajik · Тоҷикӣ | Cyrl | – | – | – | egui | 2, verify | 1 |
| `uz` | Uzbek · Oʻzbekcha | Latn | – | – | ✓ | shipped | 2 (one = 1) | 1 |
| `zu` | Zulu · isiZulu | Latn | – | – | – | egui | 2 (0 and 1) | 1 |
| `bn` | Bengali · বাংলা | Beng | – | – | ✓ | pin bump | 2 (0 and 1) | 2 |
| `fa` | Persian · فارسی | Arab RTL | – | – | ✓ | shipped | 2 (0 and 1) | 2 |
| `gn` | Guarani · Guarani | Latn | – | – | – | shipped | unknown, verify | 2 |
| `he` | Hebrew · עברית | Hebr RTL | – | – | ✓ | shipped | new rule | 2 |
| `hi` | Hindi · हिन्दी | Deva | – | – | ✓ | shipped | 2 (0 and 1) | 2 |
| `is` | Icelandic · Íslenska | Latn | – | – | – | egui | new rule | 2 |
| `lo` | Lao · ລາວ | Laoo | – | – | – | pin bump | 1 form | 2 |
| `lt` | Lithuanian · Lietuvių | Latn | – | – | ✓ | egui | new rule | 2 |
| `lv` | Latvian · Latviešu | Latn | – | – | ✓ | egui | new rule | 2 |
| `mk` | Macedonian · Македонски | Cyrl | – | – | – | egui | new rule | 2 |
| `mr` | Marathi · मराठी | Deva | – | – | ✓ | shipped | 2 (one = 1) | 2 |
| `ne` | Nepali · नेपाली | Deva | – | – | – | shipped | 2 (one = 1) | 2 |
| `ro` | Romanian · Română | Latn | – | – | ✓ | egui | new rule | 2 |
| `sl` | Slovenian · Slovenščina | Latn | – | – | ✓ | egui | new rule | 2 |
| `te` | Telugu · తెలుగు | Telu | – | – | ✓ | pin bump | 2 (one = 1) | 2 |
| `th` | Thai · ไทย | Thai | – | – | ✓ | shipped | 1 form | 2 |
| `ur` | Urdu · اردو | Arab RTL | – | – | ✓ | shipped | 2 (one = 1) | 2 |
| `am` | Amharic · አማርኛ | Ethi | – | – | – | none | 2 (0 and 1) | 3 |
| `dv` | Dhivehi · ދިވެހި | Thaa RTL | – | – | – | none | 2 (one = 1) | 3 |
| `hy` | Armenian · Հայերեն | Armn | – | – | – | none | 2 (0 and 1) | 3 |
| `km` | Khmer · ខ្មែរ | Khmr | – | – | – | none | 1 form | 3 |
| `my` | Burmese · မြန်မာ | Mymr | – | – | ✓ | none | 1 form | 3 |
| `ps` | Pashto · پښتو | Arab RTL | – | – | – | none (2 letters) | 2 (one = 1) | 3 |
| `si` | Sinhala · සිංහල | Sinh | – | – | – | none | 2 (0 and 1) | 3 |
| `pt-PT` | Portuguese (Portugal) · Português (Portugal) | Latn | – | – | ✓ | egui | 2 (one = 1) | V |

Counts of Studio's 75: covered by us 22, wave 1: 28, wave 2: 17, wave 3: 7, variant 1 (22 + 28 + 17 + 7 + 1 = 75).

## What blocks a language, concretely

- **Fonts (wave 3).** No `craft-fonts` face covers Amharic (Ethiopic), Dhivehi (Thaana), Armenian, Khmer, Burmese (Myanmar) or Sinhala, and two Pashto
  letters. Noto Sans has all of them under the SIL OFL; they have to be added to `craft-fonts` (or embedded here) and the web build's size budget
  checked (Cloudflare Pages: 25 MiB per file).
- **Shaping and bidi.** The egui fork we pin includes `harfrust` (HarfBuzz) and `unicode-bidi`, so complex scripts and right-to-left text are
  handled at the text layer. Whether the whole UI lays out correctly right-to-left (Hebrew, Persian, Urdu, Pashto, Dhivehi) beyond what Arabic already does, and
  how Devanagari, Bengali, Thai and Lao look in our panels, has not been checked; each needs a screenshot test before its catalog ships.
- **Plural rules (wave 2).** New rules are needed for Lithuanian, Latvian, Slovenian, Romanian, Hebrew, Macedonian and Icelandic; Guarani needs a lookup; Filipino
  and Tajik should be checked. Everything else maps to a rule we already have.
- **Variants.** Spanish: our `es` serves every `es-*` tag, so Studio's es-ES and es-MX are covered. Portuguese: `pt` serves Brazil only; Portugal (`pt-PT`)
  would be a second catalog. Decide whether to split.
- **Size.** Our 21 catalogs are 1.4 MB (about 67 KB each; Indic scripts 120 KB or more). 53 more would add roughly 4 MB to a build, and to the web build too
  unless catalogs load on demand.
- **Review.** The repository already records AI-assisted catalogs with a note that no native speaker has reviewed them (`ATTRIBUTION.md`). The same row
  wording works for these, but the more languages we ship, the more we ship unreviewed text.

## Per language, the same steps (as done for pl, ka and ta)

1. Catalog `crates/ui-egui/src/i18n/<code>.tsv` (context, English, translation columns; plurals as `one|other...`), and an `ATTRIBUTION.md` row: author, source, licence. Rows taken from Studio's packs keep **Apache-2.0** (not MIT OR Apache-2.0) and name the pack and its version.
2. A `Preference` variant, its code, its native name (the English name where the script cannot be drawn, as Korean does), the `LANGUAGES` entry with its plural function, and the locale tags that select it.
3. A plural function if the language needs a new rule, with a test.
4. Fonts: a face in `craft-fonts` (and `CRAFT_FONTS_REF` raised), the `can_show` arm and `embedded_ui_faces` entry for the script, and for the web build the script listed in `crates/fonts/build.rs`.
5. Tests: locale aliases, preference parse, plural forms, "every letter of the catalog has a glyph".
6. Docs: `docs/localization.md`, README count, CHANGELOG line.

## Proposed next steps

1. **`cargo xtask i18n-seed <pack-locale>`** (Rust, in `xtask`): hash our English strings, take Studio's matching entries for that language into a draft `<code>.tsv`
   marked as a draft, and write the `ATTRIBUTION.md` row. It makes steps 1 of the 33 pack languages mechanical: 485 strings each, and a list of the rest to translate.
2. **Wave 1, starting with Japanese**: Japanese faces already ship, it has one plural form, and AutoCAD itself is localized into it, so it is the commercial
   priority. AutoCAD's own localizations are Chinese (both), Czech, French, German, Hungarian, Italian, Japanese, Korean, Polish, Portuguese (Brazil), Russian and Spanish
   (from public product information, not checked today): once Japanese is in, we cover all of them.
3. **Raise `CRAFT_FONTS_REF`** to `83c36c8` or later so Tamil, Bengali and Telugu (and Lao, at `baf0f2c`) show without an installed font. Upstream has not done this either.
4. **Add the six missing scripts** to `craft-fonts` before any wave-3 language.
5. **Native review**: find a reviewer per language before it ships as anything but a draft; Studio's own files say none of theirs has one.
