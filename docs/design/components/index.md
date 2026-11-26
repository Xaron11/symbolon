# Components

Each page specifies one component independently of its implementation: anatomy, variants, sizes (as [tokens](../tokens.md)), states, keyboard behaviour and accessibility. The desktop app implements them in `crates/symbolon-app/ui/src/lib/components/`, and each source file links back to its page.

States share one priority when several apply: disabled, loading, pressed, focus, hover. Disabled controls use `opacity.disabled` and a not-allowed cursor.
