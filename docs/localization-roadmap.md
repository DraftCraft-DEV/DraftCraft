# Interface language roadmap

Written 2026-11-26, updated 2026-11-27. The aim: a route to as many languages as CadCraft Studio offers, its 75 being the goalpost
for breadth only. What is measured is marked as measured; the rest is marked as reading.

## Where we are

| | Languages besides English | Source |
|---|---|---|
| DraftCraft | **49**: the 21 below plus the 28 of wave 1 | `crates/ui-egui/src/i18n.rs` |
| Before wave 1 | 21: ar bs cs de el es fr hu it ka ko pl pt-BR ru sv ta tr uk vi zh zh-TW | |
| CadCraft Studio (`getcadcraft/studio`, `hub/lang`) | **75** (76 files with `en`) | 0.11.0, `8fd681c` (2026-11-23) |

Of Studio's 75, we now cover 50 (our one Spanish stands for both of its Spanish variants); 25 remain: 17 in wave 2, 7 in wave 3, and
Portuguese (Portugal).

## Wave 1: shipped, waiting for native review

The 28 wave-1 languages are in (`af az be bg ca da et fi fil gl hr id ja kk ky mn ms nb nl sk so sq sr sr-Latn sw tg uz zu`).
They were written from DraftCraft's own English strings by WithShalom (Shalom) with Claude Code. Nothing was taken from CadCraft Studio
or any other product: Studio's 75 languages are only the target for how many languages we cover. Each catalog has the same 1,688 rows,
passes the catalog tests (placeholders, ellipses, plural forms, glyph coverage) and falls back to English only where a string is new.

### Limitations

- **No native speaker has reviewed any of them.** They are drafts that read correctly to a careful non-native writer, not finished
  translations. Treat terminology as provisional until a reviewer signs off.
- **CAD vocabulary is thin in several languages.** Swahili, Somali, Zulu, Kyrgyz and Tajik have little established CAD terminology;
  terms such as layer, viewport, hatch, snap and grip were coined or borrowed and need a practitioner's choice.
- **Loanwords vary by language.** Filipino keeps about 350 strings in English (Layer, Block, Hatch, Viewport...), Indonesian about 100,
  Danish about 100. That follows common usage but should be confirmed.
- **Croatian and Serbian (Latin)** started from our Bosnian catalog and were converted word by word, so some Bosnian forms may remain.
- **Japanese** shows only with a Japanese or CJK font (installed, or a craft-fonts Japanese face); without one the interface stays in
  English and the status line says why. Its layout in narrow ribbon panels has not been checked on screen.
- **Azerbaijani** shows only with an installed interface font that has ə (Segoe UI, Arial, SF, DejaVu Sans, Noto Sans); egui's bundled faces lack it, so the web build stays in English for Azerbaijani until a font is bundled.
- **Uzbek** uses ‘ (U+2018) rather than the official ʻ (U+02BB) for oʻ and gʻ, because the bundled fonts lack U+02BB. This is common in everyday Uzbek text, but a native speaker may prefer the official letter once a font is bundled.
- **The single-letter row "B"** (beside the compass letters E, N, S, W) was translated without context; translators guessed
  "bottom" or "back". It needs checking against the ViewCube or compass where it appears.
- **Not checked on screen:** long labels in the ribbon and dialogs for languages with long words (Finnish, Estonian, Zulu).

### Requests for native speakers

Please open the catalog in `crates/ui-egui/src/i18n/<code>.tsv`, switch the interface to your language (Edit → Interface Language)
and correct anything that reads wrong. Points the translators flagged:

| Language | Please check |
|---|---|
| Afrikaans `af` | "aansig" is used for both View and Viewport; vasmaak (snap), opsporing (tracking), bladstel (sheet set) |
| Albanian `sq` | Erase "Fshij" vs Delete "Fshi"; Sweep also uses "Fshij"; faqosje (layout), vijëzim (hatch) |
| Azerbaijani `az` | maket (layout), görünüş pəncərəsi (viewport), bağlama (snap), ştrix (hatch), descriptive Sweep/Revolve |
| Belarusian `be` | слой vs пласт (layer), відавы экран (viewport), пункт vs кропка (point), DesignCenter rendered literally |
| Bulgarian `bg` | прозорец на изгледа (viewport) and its short form ПИ, износка (leader), Английски (imperial units) |
| Catalan `ca` | Sombreig vs Tramat (hatch), finestra gràfica (viewport) and FG, Forçament (snap), Equidistància (offset) |
| Croatian `hr` | leftover Bosnian forms ("da dodate"), raspored vs izgled (layout), šrafura, Odsijeci vs Odreži (trim) |
| Danish `da` | "Ø" is both Island and East; dimension/snap/layout/array kept as loanwords; Kote (elevation) |
| Dutch `nl` | viewport, snap, grip, tracking, extents kept in English; onderlegger (underlay), bladenset (sheet set) |
| Estonian `et` | South and West both "L"; side (constraint), haare (snap), viiktekst (leader) |
| Filipino `fil` | about 350 English loanwords kept; plural "{n} na commit" may read unnaturally |
| Finnish `fi` | Array and Table are both "Taulukko"; kohdistus (snap), viitta (leader), monoviiva (multiline) |
| Galician `gl` | "Capas da capa subxacente" (Underlay Layers) reads awkwardly; Modelo (template); infinitive commands |
| Indonesian `id` | many CAD terms kept in English (layer, viewport, polyline, offset, fillet); West and "B" both "B" |
| Japanese `ja` | レイヤー rather than 画層 for layer; ribbon label lengths; その他 for "Grab bag" |
| Kazakh `kk` | өрнек for both Pattern and Expression; сілтеме сызығы (leader) close to "reference"; DesignCenter |
| Kyrgyz `ky` | байланыш (constraint) also means connection; подложка (underlay) is a Russian loan; compass abbreviations |
| Malay `ms` | Conic and Cone are both "Kon"; "Lapisan bagi Lapisan Bawah" (Underlay Layers); Lorek (hatch) |
| Mongolian `mn` | товчлол is both Alias and Shortcut; харах цонх (viewport), наалдалт (snap), штрих (hatch) |
| Norwegian Bokmål `nb` | mål/målsettingsstil (dimension), fang (snap), oppsett (layout), matrise (array) |
| Serbian Cyrillic `sr` | распоред vs лист (layout), прозор приказа (viewport) and ПП, ручица (grip), маска (wipeout) |
| Serbian Latin `sr-Latn` | leftover Bosnian forms ("Kad" vs "Kada"), Upravljač vs Menadžer, Ostava (clipboard) |
| Slovak `sk` | výrez (viewport), rozvrhnutie (layout), uzly for both grips and Node; Šablónovanie (loft), Ťahanie (sweep) |
| Somali `so` | coined terms: xariijin (hatch), xarig-badan (polyline), daaqad-muuqaal (viewport), ku-dhejin (snap) |
| Swahili `sw` | coined terms: tabaka (layer), dirisha-tazamo (viewport), kunasa (snap); compass letters clash (M/K) |
| Tajik `tg` | равзанаи намоиш (viewport), пайвастшавӣ (snap), тарҳбандӣ (layout); "clean-room" rendered loosely |
| Uzbek `uz` | maket (layout), bogʻlash (snap) vs bogʻlanish (constraint), shtrixovka (hatch) |
| Zulu `zu` | ungqimba (layer), isakhiwo for both layout and properties, Khipha for Unload and Subtract, Hlehlisa for Undo, Reverse and Zoom Out |

When a language has been reviewed, its `ATTRIBUTION.md` row and the catalog header say so, and the reviewer is credited there.

## Why we do not take Studio's translations

DraftCraft's catalogs are written from DraftCraft's own English strings, in DraftCraft's own format. CadCraft Studio is a separate
application with an unrelated architecture; we take no code and no translations from it. Measured on 2026-11-26, its packs would also
have helped little: they match only 29% of our strings (the rest are AutoCAD-style commands and prompts it does not have), they are
unreviewed machine drafts, and where both sides have a string they agree with our existing catalogs only 42% to 82% of the time.

## Languages

Column key. **Font**: `egui` = egui's bundled face covers every letter Studio's text uses in that language; `shipped` = a face in the `craft-fonts`
revision our releases pin (`b076417b`); `pin bump` = a face exists in craft-fonts `83c36c8` or later, so raising `CRAFT_FONTS_REF` fixes it
(+4.3 MB on desktop for the eight new Indic faces); `none` = no `craft-fonts` face covers the script. Checked by testing each language's letters against the faces
themselves, on Studio's launcher text plus its CAD-app strings where a pack exists (a sample, not our full UI).
**Plurals** use the rules we already have, my reading of CLDR; verify before implementing. **Wave**: 1 = can go on today's code; 2 = one small
addition first (a plural rule, a pin bump, or a right-to-left or complex-script rendering check); 3 = needs a font we do not have; V = variant of a language we have.

| Code | Language | Script | Ours | Studio packs | Font | Plurals | Wave |
|---|---|---|:-:|:-:|---|---|:-:|
| `ar` | Arabic · العربية | Arab RTL | ✓ | ✓ | shipped | 6 (ar rule) | — |
| `bs` | Bosnian · Bosanski | Latn | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | — |
| `cs` | Czech · Čeština | Latn | ✓ | ✓ | egui | 3 (cs rule) | — |
| `de` | German · Deutsch | Latn | ✓ | ✓ | egui | 2 (one = 1) | — |
| `el` | Greek · Ελληνικά | Grek | ✓ | ✓ | egui | 2 (one = 1) | — |
| `es-ES` | Spanish (Spain) · Español (España) | Latn | ✓ | ✓ | egui | 2 (one = 1) | — |
| `es-MX` | Spanish (Mexico) · Español (México) | Latn | ✓ | ✓ | egui | 2 (one = 1) | — |
| `fr` | French · Français | Latn | ✓ | ✓ | egui | 2 (0 and 1) | — |
| `hu` | Hungarian · Magyar | Latn | ✓ | ✓ | egui | 1 form | — |
| `it` | Italian · Italiano | Latn | ✓ | ✓ | egui | 2 (one = 1) | — |
| `ka` | Georgian · ქართული | Geor | ✓ | – | shipped | 1 form | — |
| `ko` | Korean · 한국어 | Kore | ✓ | ✓ | shipped | 1 form | — |
| `pl` | Polish · Polski | Latn | ✓ | ✓ | egui | 3 (pl rule) | — |
| `pt-BR` | Portuguese (Brazil) · Português (Brasil) | Latn | ✓ | ✓ | egui | 2 (0 and 1) | — |
| `ru` | Russian · Русский | Cyrl | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | — |
| `sv` | Swedish · Svenska | Latn | ✓ | ✓ | egui | 2 (one = 1) | — |
| `ta` | Tamil · தமிழ் | Taml | ✓ | ✓ | pin bump | 2 (one = 1) | — |
| `tr` | Turkish · Türkçe | Latn | ✓ | ✓ | egui | 1 form | — |
| `uk` | Ukrainian · Українська | Cyrl | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | — |
| `vi` | Vietnamese · Tiếng Việt | Latn | ✓ | ✓ | shipped | 1 form | — |
| `zh-Hans` | Chinese (Simplified) · 简体中文 | Hans | ✓ | ✓ | shipped | 1 form | — |
| `zh-Hant` | Chinese (Traditional) · 繁體中文 | Hant | ✓ | ✓ | shipped | 1 form | — |
| `af` | Afrikaans · Afrikaans | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `az` | Azerbaijani · Azərbaycanca | Latn | ✓ | – | shipped | 2 (one = 1) | ✓ (wave 1) |
| `be` | Belarusian · Беларуская | Cyrl | ✓ | – | egui | 3 (1 · 2-4 · rest) | ✓ (wave 1) |
| `bg` | Bulgarian · Български | Cyrl | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `ca` | Catalan · Català | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `da` | Danish · Dansk | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `et` | Estonian · Eesti | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `fi` | Finnish · Suomi | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `fil` | Filipino · Filipino | Latn | ✓ | ✓ | egui | 2, verify | ✓ (wave 1) |
| `gl` | Galician · Galician | Latn | ✓ | – | egui | 2 (one = 1) | ✓ (wave 1) |
| `hr` | Croatian · Hrvatski | Latn | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | ✓ (wave 1) |
| `id` | Indonesian · Bahasa Indonesia | Latn | ✓ | ✓ | egui | 1 form | ✓ (wave 1) |
| `ja` | Japanese · 日本語 | Jpan | ✓ | ✓ | shipped | 1 form | ✓ (wave 1) |
| `kk` | Kazakh · Қазақша | Cyrl | ✓ | – | egui | 2 (one = 1) | ✓ (wave 1) |
| `ky` | Kyrgyz · Кыргызча | Cyrl | ✓ | – | egui | 2 (one = 1) | ✓ (wave 1) |
| `mn` | Mongolian · Монгол | Cyrl | ✓ | – | egui | 2 (one = 1) | ✓ (wave 1) |
| `ms` | Malay · Bahasa Melayu | Latn | ✓ | ✓ | egui | 1 form | ✓ (wave 1) |
| `nb` | Norwegian Bokmål · Norsk bokmål | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `nl` | Dutch · Nederlands | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `sk` | Slovak · Slovenčina | Latn | ✓ | ✓ | egui | 3 (cs rule) | ✓ (wave 1) |
| `so` | Somali · Soomaali | Latn | ✓ | – | egui | 2 (one = 1) | ✓ (wave 1) |
| `sq` | Albanian · Shqip | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `sr-Cyrl` | Serbian (Cyrillic) · Српски (ћирилица) | Cyrl | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | ✓ (wave 1) |
| `sr-Latn` | Serbian (Latin) · Srpski (latinica) | Latn | ✓ | ✓ | egui | 3 (1 · 2-4 · rest) | ✓ (wave 1) |
| `sw` | Swahili · Kiswahili | Latn | ✓ | ✓ | egui | 2 (one = 1) | ✓ (wave 1) |
| `tg` | Tajik · Тоҷикӣ | Cyrl | ✓ | – | egui | 2, verify | ✓ (wave 1) |
| `uz` | Uzbek · O‘zbekcha | Latn | ✓ | ✓ | shipped | 2 (one = 1) | ✓ (wave 1) |
| `zu` | Zulu · isiZulu | Latn | ✓ | – | egui | 2 (0 and 1) | ✓ (wave 1) |
| `bn` | Bengali · বাংলা | Beng | – | ✓ | pin bump | 2 (0 and 1) | 2 |
| `fa` | Persian · فارسی | Arab RTL | – | ✓ | shipped | 2 (0 and 1) | 2 |
| `gn` | Guarani · Guarani | Latn | – | – | shipped | unknown, verify | 2 |
| `he` | Hebrew · עברית | Hebr RTL | – | ✓ | shipped | new rule | 2 |
| `hi` | Hindi · हिन्दी | Deva | – | ✓ | shipped | 2 (0 and 1) | 2 |
| `is` | Icelandic · Íslenska | Latn | – | – | egui | new rule | 2 |
| `lo` | Lao · ລາວ | Laoo | – | – | pin bump | 1 form | 2 |
| `lt` | Lithuanian · Lietuvių | Latn | – | ✓ | egui | new rule | 2 |
| `lv` | Latvian · Latviešu | Latn | – | ✓ | egui | new rule | 2 |
| `mk` | Macedonian · Македонски | Cyrl | – | – | egui | new rule | 2 |
| `mr` | Marathi · मराठी | Deva | – | ✓ | shipped | 2 (one = 1) | 2 |
| `ne` | Nepali · नेपाली | Deva | – | – | shipped | 2 (one = 1) | 2 |
| `ro` | Romanian · Română | Latn | – | ✓ | egui | new rule | 2 |
| `sl` | Slovenian · Slovenščina | Latn | – | ✓ | egui | new rule | 2 |
| `te` | Telugu · తెలుగు | Telu | – | ✓ | pin bump | 2 (one = 1) | 2 |
| `th` | Thai · ไทย | Thai | – | ✓ | shipped | 1 form | 2 |
| `ur` | Urdu · اردو | Arab RTL | – | ✓ | shipped | 2 (one = 1) | 2 |
| `am` | Amharic · አማርኛ | Ethi | – | – | none | 2 (0 and 1) | 3 |
| `dv` | Dhivehi · ދިވެހި | Thaa RTL | – | – | none | 2 (one = 1) | 3 |
| `hy` | Armenian · Հայերեն | Armn | – | – | none | 2 (0 and 1) | 3 |
| `km` | Khmer · ខ្មែរ | Khmr | – | – | none | 1 form | 3 |
| `my` | Burmese · မြန်မာ | Mymr | – | ✓ | none | 1 form | 3 |
| `ps` | Pashto · پښتو | Arab RTL | – | – | none (2 letters) | 2 (one = 1) | 3 |
| `si` | Sinhala · සිංහල | Sinh | – | – | none | 2 (0 and 1) | 3 |
| `pt-PT` | Portuguese (Portugal) · Português (Portugal) | Latn | – | ✓ | egui | 2 (one = 1) | V |

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

1. Catalog `crates/ui-egui/src/i18n/<code>.tsv` (context, English, translation columns; plurals as `one|other...`), and an `ATTRIBUTION.md` row: author, source, licence.
2. A `Preference` variant, its code, its native name (the English name where the script cannot be drawn, as Korean does), the `LANGUAGES` entry with its plural function, and the locale tags that select it.
3. A plural function if the language needs a new rule, with a test.
4. Fonts: a face in `craft-fonts` (and `CRAFT_FONTS_REF` raised), the `can_show` arm and `embedded_ui_faces` entry for the script, and for the web build the script listed in `crates/fonts/build.rs`.
5. Tests: locale aliases, preference parse, plural forms, "every letter of the catalog has a glyph".
6. Docs: `docs/localization.md`, README count, CHANGELOG line.

## Proposed next steps

1. **Native review of wave 1** (see the table above), one language at a time, crediting the reviewer in `ATTRIBUTION.md`.
2. **Wave 2** in its own pull request: the plural rules for Lithuanian, Latvian, Slovenian, Romanian, Hebrew, Macedonian and Icelandic
   (and a lookup for Guarani), right-to-left checks for Hebrew, Persian and Urdu, and screenshots for Devanagari, Bengali, Telugu, Thai and Lao.
3. **Raise `CRAFT_FONTS_REF`** to `83c36c8` or later so Tamil, Bengali and Telugu (and Lao, at `baf0f2c`) show without an installed font.
4. **Wave 3** after the six missing scripts (Ethiopic, Thaana, Armenian, Khmer, Myanmar, Sinhala) are added to `craft-fonts`.
5. **Portuguese (Portugal)** as a second Portuguese catalog, if we decide to split `pt`.
