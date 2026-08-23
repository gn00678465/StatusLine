use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn renders_fallback_for_json_input() -> Result<(), Box<dyn Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-statusline"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"{}")?;
    }

    let output = child.wait_with_output()?;

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout)?, "Claude\n");

    Ok(())
}

#[test]
fn renders_fallback_for_empty_input() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_cc-statusline")).output()?;

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout)?, "Claude\n");

    Ok(())
}
