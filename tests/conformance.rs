//! Conformance corpus: compiles + runs each `tests/conformance/*.vd` via the native
//! backend and checks its stdout against the matching `*.expected` golden file.
//!
//! Skips automatically where the native backend isn't available (Windows, or no clang),
//! so it never fails a toolchain-less CI; in WSL/Linux with clang it runs for real.

use std::path::Path;
use std::process::Command;

#[test]
fn conformance_corpus() {
    if cfg!(target_os = "windows") {
        eprintln!("conformance: skipped on Windows (native backend needs WSL)");
        return;
    }
    if Command::new("clang").arg("--version").output().is_err() {
        eprintln!("conformance: skipped (clang not found on PATH)");
        return;
    }

    let vader = env!("CARGO_BIN_EXE_vader");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");

    let mut programs: Vec<_> = std::fs::read_dir(&dir)
        .expect("read tests/conformance")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |x| x == "vd"))
        .collect();
    programs.sort();
    assert!(!programs.is_empty(), "no conformance programs found");

    let mut failures = Vec::new();
    for vd in &programs {
        let expected = std::fs::read_to_string(vd.with_extension("expected"))
            .unwrap_or_else(|_| panic!("missing .expected for {}", vd.display()));

        let out = Command::new(vader)
            .arg("llvm")
            .arg(vd)
            .output()
            .expect("failed to run vader");
        let stdout = String::from_utf8_lossy(&out.stdout);
        // the program's own output follows the "--- running ---" marker
        let got = stdout.split("--- running ---\n").nth(1).unwrap_or("");

        if got.trim_end() != expected.trim_end() {
            failures.push(format!(
                "{}\n   expected: {:?}\n   got:      {:?}",
                vd.file_name().unwrap().to_string_lossy(),
                expected.trim_end(),
                got.trim_end()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "conformance regressions ({} program(s)):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
