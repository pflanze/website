Possible optimizations:

  * The database carries copies of the attributes for every elements!
    Share those instead (after verifying they are the same, or is the
    code currently copying them from a single source?).
    
  * The lazy_static globals like `A_META` in
    `ahtml_elements_include.rs` are getting their values out of the
    big hashmap. Is that hashmap not even needed? Could the individual
    sets inside ElementMeta use static data structures and everything
    could be static? But then what does this buy other than a couple
    MB of RAM during startup.
