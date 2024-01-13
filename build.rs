extern crate pkg_config;

fn main() {
    // println!("cargo:rustc-link-lib=static=sqlite3");
    pkg_config::Config::new().atleast_version("3.40.1").probe("sqlite3").unwrap();
}
