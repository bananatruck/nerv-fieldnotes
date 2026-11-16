# Native Fieldnotes design

The app opens one compact sticky with a plain text writing area.
The title, tags, paper color, checklist mode, and other controls live in the top-right dots menu.
The canvas has no visible title field, options expander, or permanent footer.
Save errors can appear without adding a permanent status row.
Rust supplies six-pixel margins around the writing area.
The body uses 14-pixel text with a 1.2 line height.
The native header stays near 28 pixels high.

The full board uses the visual reference supplied by Kesh for the application frame.
Its charcoal background, subtle red rules, green actions, and compact labels carry that identity.
The board has a small Fieldnotes title rather than a large masthead.
Cards use ten-pixel padding without a forced minimum height.
The writing window keeps a neutral paper surface so the frame does not compete with note text.

`style.css` uses native GTK4 CSS.
Rust owns widget layout, visibility, resizing, and scroll behavior.
The stylesheet needs no browser engine, network fonts, or image assets.

The rewritten stylesheet was loaded with the installed GTK 4.24 CSS parser.
It produced zero warnings or errors.
Integrated widget geometry and interaction checks belong to the native app verification report.
