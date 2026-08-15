use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Report a missing explicit config as a failed process.
#[test]
fn missing_config_exits_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_dwm_status"))
        .args(["--config", "/definitely/missing/dwm-status.toml"])
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success());
    assert!(stderr.contains("Failed to read /definitely/missing/dwm-status.toml"));
}

/// Report malformed TOML as a failed process.
#[test]
fn invalid_toml_exits_nonzero() {
    let output = _run_with_config("features = [\n");
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success());
    assert!(stderr.contains("Error in /dev/stdin"));
}

/// Report unsupported feature names as a failed process.
#[test]
fn unsupported_feature_exits_nonzero() {
    let output = _run_with_config("features = [\"bogus\"]\n");
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success());
    assert!(stderr.contains("Unsupported features: bogus"));
}

/// Validate clock timezones before attempting an X connection.
#[test]
fn invalid_timezone_exits_nonzero() {
    let output = _run_with_config(
        "features = [\"clock\"]\n\n[clock]\ntimezone = \"Mars/Olympus_Mons\"\n",
    );
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success());
    assert!(stderr.contains("Unsupported clock timezone: Mars/Olympus_Mons"));
}

/// Report an unavailable X session as a failed process.
#[test]
fn missing_display_exits_nonzero() {
    let output = _run_with_config("features = [\"clock\"]\n");
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success());
    assert!(stderr.contains("Failed to connect to X11"));
}

/// Run one config through the real binary without an X display.
fn _run_with_config(config: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dwm_status"))
        .args(["--config", "/dev/stdin"])
        .env_remove("DISPLAY")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(config.as_bytes()).unwrap();

    child.wait_with_output().unwrap()
}
