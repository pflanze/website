# My website

This is the code that serves my website at `christianjaeger.ch`. It
also has the functionality to serve a blog, and has a preview for blog
posts with user logins.

It's a work in progress. YMMV. I might work to make this more
reusable, and publish a blog post about it.

## Installation

Note: I'm using older versions of dependencies (which I generally do
so that the code compiles with rustc from Debian stable, and to reduce
the risk from developer or server system breaches). The code
indirectly depends on ring 0.16.20, which looks like it might use
precompiled binaries, so I'm using `cargo vendor` and rebuild the
binaries in `ring` from source locally before building the
website. You could also `cargo update` instead and adapt the code if
necessary, since recent `ring` explicitly only uses precompiled
binaries on Windows. Or you could trust the systems where those
binaries were built, of course.

I have modified the `blake3` crate to use an older version of
`constant_time_eq` that compiles with rustc from Debian stable, and
`argon2` and `addr2line` to not require a newer rust-version than
Debian's compiler (which works just fine, but at least `argon2`
upstream doesn't want to apply the change). They are expected to be
checked out at `../src/`. So, run:

    cd ..
    mkdir src
    cd src
    git clone https://github.com/pflanze/BLAKE3
    git clone https://github.com/pflanze/password-hashes
    ( cd password-hashes && git checkout -b local cj3 )
    git clone https://github.com/pflanze/addr2line

These repositories, just like this one, carry Git tags signed by
me.

## Configuration

### `example`

Doesn't need anything, just connect to port 3000 on localhost.

### `website`

The `website` program reads a number of environment variables:

    export IS_DEV=1  # do not listen on low port numbers, do not expect TLS keys
    export SESSIONID_HASHER_SECRET=$your_secret_string
    export TLSKEYSFILEBASE=... # optional base path to TLS files; .crt and .key are appended
    export WWWDIR=data/fallback/  # optional fallback for serving static files
    export WELLKNOWNDIR=~/.well-known/ # optional dir for Let's encrypt to fetch files from

`SESSIONID_HASHER_SECRET` is used as input to hash session ids before
storing them in the database, to avoid potential side channel attacks
on them. Any random string works but needs to stay the same across
restarts so that sessions from before the restart continue to work,
and should be random enough to serve its purpose (e.g. `head -c 20
/dev/urandom |base64`).

These dirs need to exist (you could also use symlinks):

    mkdir data/{blog,preview}

The `accounts.db` sqlite database file needs to exist. Create via 

    sqlite3 accounts.db < accounts-schema.sql

Access to the `/preview` and `/fellowship` paths is restricted. Run
`cargo run --bin access_control -- --help` for how to create a group,
users, and adding the users to the group. As a minimum:

    cargo build --bin access_control
    target/debug/access_control create-group --group preview
    target/debug/access_control create-group --group fellowship

Then for each user (replace `$USER` with your desired username if it's
not your unix user name):

    target/debug/access_control create-user --user $USER
    target/debug/access_control add --group preview --user $USER

There's currently no rate limiting, so use good passwords (it does
slow down each login attempt to take a second, though; hence the
minimum is about 56 bits of entropy).

Run the server via `cargo run --release --bin website`, it expects TLS
keys (set the `TLSKEYSFILEBASE` env var) and to bind on low port
numbers unless `IS_DEV=1` is set.

## Debugging

### Tracing HTML origin

When the `AHTML_TRACE` env var is set to a true value or the empty
string, every HTML element that doesn't already have a `title`
attribute will gain one that contains a backtrace from the location
where the constructor for that element is called. This allows to trace
back to the code that generated the element in question. Important:
build in debug mode or the backtrace will be useless.

## License

The `*.asc` files (public key) are public domain, everything else is ©
Christian Jaeger <ch@christianjaeger.ch> and licensed under MIT OR
Apache-2.0 at your choice.
