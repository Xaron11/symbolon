# Combobox

Picks one value from a long list by typing, such as a location.

**Anatomy:** visible label; text field; floating list of matching options, or "No matches".

**Behaviour:** options filter as the user types (by default, the label contains the text, ignoring case), and the first match is highlighted, so Enter picks it. ↑/↓ move through matches; Escape closes and clears the typed filter.

**Accessibility:** the field is a `combobox` named by its label, with a `listbox` of `option`s.
