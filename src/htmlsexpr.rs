//! Html templating system based on S-expressions via anysexpr.

//! Everything runtime (interpretative) for now, later on preprocess
//! templates to make fragments.

//! Want to be able to change templates on disk without
//! recompiling. And proper type safety (at runtime, or preprocess
//! time in the future). With just warnings for now? Or, right now,
//! untyped, until I get this part up.

//! Control features:

//! `(def (component-name var1 ...) ..body..)`: define a new component
//! Toplevel only (no closures, for now)? The variables can be
//! accessed both by name via keyword syntax, as well as positional.

//! `(for (item v) ..body..)`: map over the items in sequence `v`,
//! defining variable `item`, evaluating `..body..`.

//! `v`: access variable as defined in component definition, or for loop.

