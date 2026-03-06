use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let hash = hash.trim().to_string();

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let exact_tag = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some();

    let suffix = if exact_tag && !dirty {
        String::new()
    } else if !hash.is_empty() {
        let d = if dirty { "-dirty" } else { "" };
        format!(".{}{}", hash, d)
    } else {
        String::new()
    };

    println!("cargo:rustc-env=GIT_VERSION_SUFFIX={}", suffix);
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}
