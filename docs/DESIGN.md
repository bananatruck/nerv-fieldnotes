# Fieldnotes design

The interface takes its cues from NERV headquarters control rooms: amber type, instrument labels, hard edges, warning orange, and a dark green-black grid.
All artwork is original CSS.
It uses system fonts and needs no font CDN, image server, or network request to draw the interface.

The notes stay warm and readable against the dark frame.
Five paper colors provide a visual memory aid without making color the only way to identify a note.
Titles, categories, tags, pin controls, timestamps, and action labels carry the meaning.

The left rail holds the main note views and secondary tools.
The search field, sort control, layout switch, and new-note button sit above the notes.
The large FIELD NOTES title gives the board a recognizable starting point without putting decoration inside the editor.

The desktop board uses three columns, becomes two columns on smaller laptops, and becomes one column on phones.
At wide desktop sizes it uses four columns.
List mode keeps previews beside titles where space permits.
The mobile navigation opens as a left drawer.

Native dialogs provide modal focus handling and Escape behavior.
Visible focus outlines support keyboard use.
Motion is limited to a small card hover and is disabled when reduced motion is requested.
Print styles remove navigation and show the full note text.

## Preview checks

Review the board at 1440, 1024, 768, and 360 pixels wide.
Check a populated board, an empty view, a long title, a long unbroken tag, an editor dialog, a handoff dialog, and the shortcut reference.
No text or controls should extend beyond the viewport.
At phone widths, confirm that the navigation toggle and close action both work.
At desktop widths, confirm that a note remains readable when the title wraps across several lines.

## Verified layout

A Chromium rendering check passed at 1440, 1024, 768, 640, and 360 pixels with long unbroken titles, body text, and tags.
The document had no horizontal overflow at those widths.
The native editor also stayed within the 360-pixel viewport.
Desktop navigation remained visible while the mobile menu button remained hidden.
The opposite visibility rules applied at 640 pixels and below.
Dark desktop, dark mobile, mobile editor, and light desktop screenshots were visually inspected.
These checks used representative note markup before the application behavior was connected.
