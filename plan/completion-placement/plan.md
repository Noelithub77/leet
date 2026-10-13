Keep the completion list and full preview outside the current typing line, including narrow panes and zoom. Size both scrolling panels to the space above or below the caret and account for padding, borders, and the Copilot cue. Use lightly translucent theme backgrounds with solid text.

Verify actual production popup bounds through isolated snippet fixtures at different caret positions, widths, and themes; inspect offscreen images. Commit only these changes and preserve other work in the checkout.

Completed: bounded window-coordinate placement, whole visible completion rows, and 98% opaque theme surfaces. The focused native snippet suite passed with six caret/layout regression cases; dark, light, zoomed, and wide offscreen images were inspected. No live desktop or installed release was changed.
