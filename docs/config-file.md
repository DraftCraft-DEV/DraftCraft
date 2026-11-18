# The settings file

One text file holds the settings you set up once and want everywhere: on another computer, after a
reinstall, or for everyone in an office. It is plain [TOML](https://toml.io), so you can read it,
edit it, keep it in version control and send it to a colleague.

| Command | Ribbon | What it does |
|---|---|---|
| CONFIGEXPORT | Manage › Customization › Export Settings | Writes your settings to a `.toml` file. |
| CONFIGIMPORT | Manage › Customization › Import Settings | Reads a `.toml` file and applies what it holds. |

Both are also in Tools › Export Settings… and Tools › Import Settings…. From the ribbon, a menu or
typed, they open a file dialog; with FILEDIA set to 0 (or where there is no file dialog) they ask
for the file name at the command line instead. In the browser, CONFIGEXPORT downloads the file.

AutoCAD keeps these settings in several places (the user profile in the registry, a `.pgp` alias
file, a template) and has no single command that writes them all to one file, so DraftCraft's names
follow its `noun + IMPORT/EXPORT` commands such as PDFIMPORT.

## What is in it

| Section | Holds | Applies to |
|---|---|---|
| `[interface]` | The app's own preferences: theme, colours, interface language and size, autosave | the app |
| `[profile]` | The Options profile: object snaps, polar tracking, grid and snap spacing, selection, grips, MEASUREINIT | the app, kept across restarts |
| `[drawing]` | MEASUREMENT, INSUNITS, LUNITS, LUPREC, AUNITS, AUPREC, ANGBASE, LTSCALE, CELWEIGHT, LWDEFAULT, DIMSCALE, TEXTSIZE, CANNOSCALE | the current drawing (one undo step) |
| `[plot]` | `plotStyleTable`, `displayPlotStyles`, `plotTransparency`, `lineweights` | every layout of the current drawing |
| `[aliases]` | Command aliases, `ALIAS = "command"` | this session, as ALIASEDIT's |

The names and values are the ones SETVAR, OPTIONS, PAGESETUP and ALIASEDIT take: a value the
file holds is checked exactly as if it were typed there. MEASUREINIT in `[profile]` makes new
drawings metric or imperial; the `[drawing]` section changes the drawing open now. To start every
new drawing with the same drawing settings, save a drawing as a template and set QNEWTEMPLATE.

## Importing

Every section and every entry is optional: what the file leaves out keeps its current value, so a
file can hold just the few settings you care about. Import applies every entry it can and lists
the rest at the command line with the reason (an unknown name, a value out of range, a plot style
table the drawing doesn't have). A file that is not TOML, is not UTF-8 text or is larger than
256 KB is refused as a whole and nothing changes.

## Example

```toml
version = 1

[interface]
theme = "dark"
modelBackground = "#1e2229"   # or "default"
interfaceScale = 1.0

[profile]
OSMODE = 4135        # endpoint, midpoint, center, intersection, extension
GRIDUNIT = [10.0, 10.0]
SNAPUNIT = [5.0, 5.0]
PICKBOX = 5
MEASUREINIT = 1      # new drawings are metric

[drawing]
MEASUREMENT = 1
INSUNITS = 4         # millimetres
LUNITS = 2           # decimal
LUPREC = 2
LTSCALE = 10.0
CELWEIGHT = -1       # ByLayer
LWDEFAULT = 25       # 0.25 mm
DIMSCALE = 1.0
TEXTSIZE = 2.5
CANNOSCALE = "1:50"

[plot]
plotStyleTable = "monochrome.ctb"
lineweights = true

[aliases]
C = "circle"
RR = "rectang"
```

## From scripts, agents and the CLI

The JSON forms never open a dialog:

```text
configexport {"file": "/path/settings.toml"}       → {text, settings, file}
configexport {}                                     → {text, settings}
configimport {"file": "/path/settings.toml"}       → {applied, issues, message}
configimport {"text": "[profile]\nPICKBOX = 5\n"}
configimport {"settings": {"profile": {"PICKBOX": 5}}}
```

`applied` lists the entries applied as `section.NAME`, `issues` the ones skipped and why. Without
the app (the CLI, the MCP server on its own) the `[interface]` section is not applied; CONFIGIMPORT
returns it as `interface` instead.
