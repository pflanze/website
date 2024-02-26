# Architecture

## Language

An enum `Lang` is used, from `lang_en_de.rs`, to catch missing cases
in `match`es. But then a trait `Language` is used from `language.rs`
in anticipation of separating the project into a library crate (which
leaves the languages open) and a website crate (which is coded against
a concrete list of languages).

A difficult case is `date_format.rs`, as that needs `match` for
concrete languages, but will be in a library, too. This is solved by
going via the string representation of the language, loses type
checking by falling back to the default for `Lang` (English).

## Various

- markdownprocessor etc. return Result, only at the very end we handle
  the errors and generate an error page. This is because why do it in
  multiple places, and 404 also needs to be handled at the outer side,
  so keep it consistent and do all errors there.
  
