# HTML metainfo database

This provides metainformation about HTML elements for correctness
checking, and is used by the `ahtml` crate.

The database about the HTML elements is linked as static structs into
the binary, from `includes/static_meta_db.rs`, which was generated
from json files copied from the html / html-sys crates,
<https://github.com/yoshuawuyts/html>.

## Static database rebuild

These json files are currently in the ahtml crate (but in the same Git
repository as this crate), in the `resources/merged/elements/`
directory.

If you need to regenerate the `static_meta_db.rs` file, currently this
hacky way is how it's done (due to be replaced with something sane):
from the Git repository root:

    WRITE_STATIC_META_DB_RS_PATH=libs/ahtml_html/includes/static_meta_db.rs HTML_READ_META_DB_FROM_JSON_DIR=libs/ahtml/resources/merged/elements/ HTML_META_DEBUG=true SESSIONID_HASHER_SECRET=1234 cargo run --bin website

then ctl-c and commit.
