use std::process::Command;

// Temporary skeleton check; replace with meaningful executable behavior tests when gameplay arrives.
#[test]
fn executable_starts_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_exploderous"))
        .output()
        .expect("failed to launch exploderous");

    assert!(output.status.success(), "executable failed: {output:?}");
    assert_eq!(output.stdout, b"Exploderous: game coming soon.\n");
}
