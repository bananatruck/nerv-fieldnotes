# Native Fieldnotes design

The native interface uses GTK4 widgets with an original theme inspired by NERV headquarters.
The dark green-black frame, amber masthead, narrow type, rust accents, and instrument labels carry the visual reference.
The note text stays dark on warm paper for long reading sessions.
The app needs no font download, bitmap asset, or browser engine to draw this theme.

## Default sticky

Opening the app shows the most recently used sticky in a compact native window.
The title and note text are ready to edit without navigating a dashboard.
The top-right dots button opens the full board when needed.
Tags, pinning, checklist mode, and color stay inside the collapsed **Note options** expander.
The footer keeps save status, forwarding, and save-and-close controls close to the note.

The sticky uses the selected paper color across its header and editor.
Title and body fields have transparent backgrounds so the window reads as one note.
The dots menu uses a dark background with light text to separate commands from note content.

## Full board

The left rail holds note views and the agent inbox.
The main area keeps search and note creation above a wrapping card board.
The 64-pixel `hero-title` uses Nimbus Sans Narrow when installed and falls back to the system sans-serif font.
Monospaced section labels and dates distinguish navigation from note content.

Paper, amber, red, green, and purple cards use explicit text colors.
Color supports recognition while titles, tags, and pin states carry meaning without color.
Buttons stay native GTK controls, including keyboard focus and disabled states.
Entry fields, text views, checkboxes, popovers, and scrollbars use the same palette.

## Native layout contract

`style.css` controls color, type, borders, padding, and focus outlines.
Rust arranges widgets with `GtkBox`, `GtkFlowBox`, and `GtkScrolledWindow`.
The theme uses GTK CSS rather than browser layout properties.
The sidebar has 17 pixels of horizontal padding and expects a width near 220 pixels.
Cards expect a natural width near 285 pixels and 18 pixels between columns and rows.
The layout can reduce the number of card columns as the window narrows.

The full board window receives the `fieldnotes` class.
The default sticky window receives `sticky-window` plus one paper color class.
The sticky content uses `sticky-content`, its title entry uses `sticky-title`, and its text view uses `sticky-body`.
Its optional controls use `sticky-options`, and its save state uses `sticky-status`.
Editor content can also receive the `editor` class.
Sticky-specific selectors override the dark editor palette.
Cards receive `note-card` plus one paper color class.
The selected navigation button receives `selected`.
Pinned controls receive `pinned`.
Main action buttons receive `primary`, and destructive buttons receive `danger`.

## Verification

The installed GTK 4.24 CSS parser loaded the stylesheet without warnings or errors.
A representative native GTK window was rendered to a PNG and visually inspected.
The board preview included the sidebar, search toolbar, three paper cards, and checklist controls.
A separate 390-by-500-pixel native sticky preview was rendered and inspected with its options collapsed.
The title, body, forwarding action, save status, and save-and-close action fit inside the window.
Application interaction and full-window layout checks belong to the native app verification report.
