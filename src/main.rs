#![deny(warnings, clippy::expect_used, clippy::unwrap_used)]

//! Entry point for the `cc-statusline` command-line application.

mod cachedir;
mod config;
mod gitstatus;
mod input;
mod oauth;
mod render;
mod ttl;
mod update;
mod width;

use std::io::Read;

fn main() {
    let mut input = String::new();
    let _read_result = std::io::stdin().read_to_string(&mut input);

    println!("Claude");
}
