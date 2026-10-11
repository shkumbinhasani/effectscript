use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TSGO: &str = env!("CARGO_BIN_EXE_tsgo");

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/effectscript")
}

fn case_dir(name: &str) -> PathBuf {
    example_dir().join("cases").join(name)
}

fn require_effect_installed() {
    let effect = example_dir().join("node_modules/effect/package.json");
    assert!(
        effect.exists(),
        "the EffectScript tests need the effect package: run `npm ci` in examples/effectscript"
    );
}

fn tsgo(dir: &Path, extra: &[&str]) -> Output {
    Command::new(TSGO)
        .current_dir(dir)
        .args(["-p", "tsconfig.json", "--pretty", "false"])
        .args(extra)
        .output()
        .expect("tsgo runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn node(dir: &Path, script: &str) -> String {
    let output = Command::new("node")
        .current_dir(dir)
        .arg(script)
        .output()
        .expect("node runs");
    let out = stdout(&output);
    assert!(
        output.status.success(),
        "node {script} failed:\n{out}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn assert_diagnostics(case: &str, expected_status: i32) {
    require_effect_installed();
    let dir = case_dir(case);
    let output = tsgo(&dir, &[]);
    assert_eq!(
        read(&dir.join("expected.txt")),
        stdout(&output),
        "diagnostics of {case}"
    );
    assert_eq!(
        Some(expected_status),
        output.status.code(),
        "exit code of {case}"
    );
}

#[test]
fn example_compiles_runs_and_is_consumed_by_plain_typescript() {
    require_effect_installed();
    let dir = example_dir();
    let output = tsgo(&dir, &[]);
    assert!(output.status.success(), "tsgo failed:\n{}", stdout(&output));
    for file in ["users.js", "users.d.ts"] {
        assert_eq!(
            read(&dir.join("expected").join(file)),
            read(&dir.join("dist").join(file)),
            "{file}"
        );
    }
    let out = node(&dir, "dist/main.js");
    assert_eq!(
        10,
        out.lines().filter(|line| line.starts_with("ok - ")).count(),
        "{out}"
    );

    let tsc = Command::new("node")
        .current_dir(&dir)
        .args([
            "node_modules/typescript/bin/tsc",
            "-p",
            "consumer/tsconfig.json",
        ])
        .output()
        .expect("tsc runs");
    assert!(tsc.status.success(), "tsc failed:\n{}", stdout(&tsc));
    let out = node(&dir, "consumer-dist/index.js");
    assert!(out.contains("ok - plain TypeScript consumer"), "{out}");
}

#[test]
fn semantic_diagnostics_check_the_effect_contract() {
    assert_diagnostics("semantic", 2);
}

#[test]
fn syntax_diagnostics_reject_unsupported_forms() {
    assert_diagnostics("syntax", 2);
}

#[test]
fn typescript_files_do_not_accept_effectscript_syntax() {
    assert_diagnostics("ts-only", 2);
}

#[test]
fn effectscript_without_effect_functions_emits_like_typescript() {
    let dir = case_dir("plain");
    let output = tsgo(&dir, &[]);
    assert!(output.status.success(), "tsgo failed:\n{}", stdout(&output));
    let out = dir.join("out");
    assert_eq!(read(&out.join("same.js")), read(&out.join("twin.js")));
    assert_eq!(read(&out.join("same.d.ts")), read(&out.join("twin.d.ts")));
    assert!(!read(&out.join("importer.js")).contains("__es_Effect"));
}

#[test]
fn commonjs_output_runs() {
    require_effect_installed();
    let dir = case_dir("cjs");
    let output = tsgo(&dir, &[]);
    assert!(output.status.success(), "tsgo failed:\n{}", stdout(&output));
    let out = node(&dir, "run.cjs");
    assert!(out.contains("ok - commonjs output"), "{out}");
}

#[test]
fn effect_modules_need_no_import() {
    require_effect_installed();
    let dir = case_dir("prelude");
    let output = tsgo(&dir, &[]);
    assert!(output.status.success(), "tsgo failed:\n{}", stdout(&output));
    let declarations = read(&dir.join("out/prelude.d.ts"));
    assert!(
        declarations.contains("): Effect.Effect<string, Missing, Config>;"),
        "{declarations}"
    );
    let out = node(&dir, "run.mjs");
    assert!(out.contains("ok - prelude names need no import"), "{out}");
}

#[test]
fn try_catch_and_finally_compose_effects() {
    require_effect_installed();
    let dir = case_dir("try");
    let output = tsgo(&dir, &[]);
    assert!(output.status.success(), "tsgo failed:\n{}", stdout(&output));
    let declarations = read(&dir.join("out/try.d.ts"));
    for expected in [
        "findOrGuest(id: string): Effect.Effect<string, DbError, never>;",
        "nested(id: string): Effect.Effect<number, never, never>;",
        "firstFound(ids: ReadonlyArray<string>): Effect.Effect<string, never, never>;",
    ] {
        assert!(
            declarations.contains(expected),
            "{expected}\n{declarations}"
        );
    }
    assert!(!declarations.contains("| undefined"), "{declarations}");
    let out = node(&dir, "run.mjs");
    assert!(out.contains("ok - try, catch and finally"), "{out}");
}
