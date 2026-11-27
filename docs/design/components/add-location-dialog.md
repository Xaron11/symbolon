# Add location dialog

Adds a folder or a phone to the catalog. Rename and remove use the same dialog layout.

**Anatomy:** tabs **Folder** and **Android device**, then the fields shared by both: Label (hint: "Defaults to the folder's name."), When to hash files ("When a comparison needs it" or "During every scan"), and a closed **Advanced** section with Hash algorithm (BLAKE3 by default, XXH3, SHA-256) and Ignore patterns (comma-separated, .gitignore syntax). The dialog opens with focus in the path field. Footer: Cancel and **Add**.

- **Folder:** a path field with **Choose…**, which opens the system's folder picker.
- **Android device:** the phones adb sees, as radio buttons with model and serial; phones that are not ready are disabled and show adb's hint (for example to allow USB debugging); **Look again** reads the list again, as does opening the tab; then the path on the phone.

**Label** follows the last part of the path until someone edits it.

**Validation** runs on Add: "Choose a folder.", "Choose a phone.", "Enter a path on the phone.", "Enter a label." under their fields. A label already in use says "A location with this label already exists." under the label. After any failure everything typed stays in the form; only the field at fault is marked (other failures show above the footer).

**Rename** asks for the new label ("Rename Photos", field "New label"); **Remove** confirms with the location's name on both buttons.

**Accessibility:** every field has a visible label; errors are linked to their fields; the dialog traps focus and Escape closes it.
