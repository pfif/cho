use std::process::Command;

fn main() {
    let opt_level = env::var("OPT_LEVEL").unwrap_or_else(|_| "0".to_string());
    let is_optimized = opt_level != "0";

    let version = if is_optimized {
        let output = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .output()
            .expect("Failed to execute git status --porcelain");

        if !output.stdout.is_empty() {
            panic!("Uncommited files exist")
        }

        let git_commit_hash = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .expect("Failed to get git commit hash")
            .stdout;

        String::from_utf8(git_commit_hash).expect("Invalid UTF-8 data")
    } else {
        "Development build".into()
    };

    println!("cargo:rustc-env=RELEASE={}", version.trim());
}
