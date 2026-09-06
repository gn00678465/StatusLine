use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Spawns the real binary with an isolated home/config so no test touches the
/// developer's real `~/.config` or triggers a network call from the update
/// checker (a fresh version cache is pre-seeded).
fn isolated_command() -> Result<(Command, TempDir, TempDir), Box<dyn Error>> {
    let home = TempDir::new()?;
    let claude_config_dir = TempDir::new()?;
    let cache_dir = home.path().join(".cache").join("StatusLine");
    std::fs::create_dir_all(&cache_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(
            home.path().join(".cache"),
            std::fs::Permissions::from_mode(0o700),
        )?;
        std::fs::set_permissions(&cache_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    std::fs::write(
        cache_dir.join("statusline-version-cache.json"),
        r#"{"tag_name":"v0.0.0"}"#,
    )?;

    let mut command = Command::new(env!("CARGO_BIN_EXE_cc-statusline"));
    command
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", claude_config_dir.path())
        .env_remove("STATUSLINE_USAGE_STYLE")
        .env_remove("STATUSLINE_GIT_CACHE_TTL")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("XDG_RUNTIME_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    Ok((command, home, claude_config_dir))
}

fn write_config_file(home: &TempDir, contents: &str) -> Result<(), Box<dyn Error>> {
    let config_dir = home.path().join("cc-statusline");
    std::fs::create_dir_all(&config_dir)?;
    std::fs::write(config_dir.join("config.toml"), contents)?;

    Ok(())
}

fn run_with_fixture(
    mut command: Command,
) -> Result<(std::process::ExitStatus, String, String), Box<dyn Error>> {
    let mut child = command.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(include_bytes!("fixtures/status-input.json"))?;
    }
    let output = child.wait_with_output()?;

    Ok((
        output.status,
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

#[test]
fn renders_dots_meter_from_config_file() -> Result<(), Box<dyn Error>> {
    let (command, home, _claude_config_dir) = isolated_command()?;
    write_config_file(&home, "usage_style = \"dots\"\n")?;

    let (status, stdout, stderr) = run_with_fixture(command)?;

    assert!(status.success());
    assert!(
        stdout.contains('\u{25cf}') || stdout.contains('\u{25cb}'),
        "stdout: {stdout}"
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");

    Ok(())
}

#[test]
fn renders_status_line_and_warns_on_malformed_config_file() -> Result<(), Box<dyn Error>> {
    let (command, home, _claude_config_dir) = isolated_command()?;
    write_config_file(&home, "usage_style = 1\n")?;

    let (status, stdout, stderr) = run_with_fixture(command)?;

    assert!(status.success());
    assert!(stdout.contains("Fable 5"), "stdout: {stdout}");
    assert!(
        stdout.contains('\u{2593}') || stdout.contains('\u{2591}'),
        "stdout: {stdout}"
    );
    assert!(stderr.contains("config.toml"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn env_overrides_config_file_in_real_binary() -> Result<(), Box<dyn Error>> {
    let (mut command, home, _claude_config_dir) = isolated_command()?;
    write_config_file(&home, "usage_style = \"dots\"\n")?;
    command.env("STATUSLINE_USAGE_STYLE", "bar");

    let (status, stdout, _stderr) = run_with_fixture(command)?;

    assert!(status.success());
    assert!(
        stdout.contains('\u{2593}') || stdout.contains('\u{2591}'),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains('\u{25cf}'), "stdout: {stdout}");

    Ok(())
}

#[test]
fn renders_fallback_for_non_json_input() -> Result<(), Box<dyn Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-statusline"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"not-json")?;
    }

    let output = child.wait_with_output()?;

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout)?, "Claude");

    Ok(())
}

#[test]
fn renders_fallback_for_empty_input() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-statusline")).output()?;

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout)?, "Claude");

    Ok(())
}
