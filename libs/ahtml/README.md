# AHTML, an HTML templating and manipulation library

This provides the following features:

  - A purely functional HTML element tree abstraction (no mutation of
    existing elements), with 32-bit id based references out of a
    specialized region based allocator, allowing efficient allocation
    and sub-tree re-use without the cost and complications of
    reference counting.
    
  - A small library of utility functions/methods to map and filter
    over the tree, allowing efficient purely functional document
    manipulations (meaning the previous (sub)tree continues to exist).

  - An element creation API that is simple enough that no custom
    language is needed (neither macro nor text-parsing
    based). I.e. "templating" happens purely via nested method calls
    of the same name as the elements one wishes to create.

  - Correctness checks for element nesting and attribute names (not
    currently values, and also not checking for the count of
    particular child elements--e.g. duplicate `title` elements in the
    `head` are not detected, mostly for performance reasons). The
    checks are done at runtime, not compile time, partially for
    pragmatism, but also to allow correctness checks when the HTML
    structure is not determined by Rust source code, but e.g. by
    parsing HTML out of a Markdown document.

  - A pre-serialization feature to convert sub-trees to their HTML
    string representation and re-use that interpretation in new trees,
    for performance (although AHTML is really quite performant
    without making use of that, too).
    
  - An (optional) allocator pool, to make allocation even more
    efficient via region re-use.

This is an alpha release, and needs proper documentation (TODO).
