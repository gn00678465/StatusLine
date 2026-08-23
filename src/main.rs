#![deny(warnings, clippy::expect_used, clippy::unwrap_used)]

//! Entry point for the `cc-statusline` command-line application.

pub mod cachedir;
mod config;
pub mod gitstatus;
mod input;
mod oauth;
pub mod render;
pub mod ttl;
mod update;
pub mod width;

use std::io::Read;

fn main() {
    let mut input = String::new();
    let _read_result = std::io::stdin().read_to_string(&mut input);

    if let Ok(parsed_input) = input::parse(&input) {
        let _status_input = (
            parsed_input.model_display_name(),
            parsed_input.effort_level(),
            parsed_input.cwd(),
            parsed_input.session_id(),
            parsed_input.context_window_size(),
            parsed_input.input_tokens(),
            parsed_input.cache_creation_input_tokens(),
            parsed_input.cache_read_input_tokens(),
            parsed_input.context_used_percentage(),
            parsed_input.five_hour_usage(),
            parsed_input.five_hour_resets_at(),
            parsed_input.seven_day_usage(),
            parsed_input.seven_day_resets_at(),
        );
    }

    let config = config::Config::from_env();
    let _config_values = (
        config.usage_style(),
        config.git_cache_ttl_seconds(),
        config.columns(),
    );

    println!("Claude");
}
