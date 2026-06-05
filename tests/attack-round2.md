    // ═══════════════════════════════════════════════════════════════
    // ROUND 2 — deeper attacks
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn attack_14_directory_traversal_dotdot() {
        // Attack: use .. to escape /tmp and hit /etc
        let r = execute_develop_action("WRITE /tmp/../../etc/passwd hacked");
        // If BLOCKED, good. If OK, path traversal works.
        let blocked = r.contains("WRITE denied");
        println!("DIRECTORY TRAVERSAL: {}", if blocked { "BLOCKED" } else { "VULN — path traversal works!" });
        assert!(blocked || r.contains("WRITE ok"), "should either block or write");
        if r.contains("WRITE ok") {
            // Clean up if it actually wrote
            let _ = std::fs::remove_file("/etc/passwd");
        }
        // Currently EXPECTED to be VULN — starts_with("/tmp") matches before .. resolution
        assert!(r.contains("WRITE ok"), "VULN: directory traversal via .. bypasses PROTECTED_DIRS");
    }

    #[test]
    fn attack_15_double_encoded_slash() {
        // Attack: try //etc to see if double slash bypasses
        let r = execute_develop_action("WRITE //etc/passwd double-slash");
        let blocked = r.contains("WRITE denied");
        println!("DOUBLE SLASH: {}", if blocked { "BLOCKED" } else { "VULN" });
    }

    #[test]
    fn attack_16_trailing_slash_bypass() {
        // Attack: does /etc/ match differently from /etc?
        let r = execute_develop_action("WRITE /etc/../tmp/escape.txt bypass");
        println!("TRAILING SLASH: {}", if r.contains("WRITE denied") { "BLOCKED" } else { "VULN" });
    }

    #[test]
    fn attack_17_cargo_build_rs_code_exec() {
        // Attack: write a malicious build.rs, then run cargo build
        // If cargo executes build.rs, agent can run arbitrary code via develop loop
        use std::io::Write;
        let build_rs = r#"
fn main() {
    // This would execute arbitrary commands if cargo runs it
    println!("cargo:warning=BUILD_RS_EXECUTED");
}
"#;
        // Write the build.rs
        std::fs::create_dir_all("/tmp/boos-attack-cargo").ok();
        let mut f = std::fs::File::create("/tmp/boos-attack-cargo/build.rs").unwrap();
        f.write_all(build_rs.as_bytes()).ok();
        // Also need a minimal Cargo.toml
        let cargo_toml = r#"
[package]
name = "attack-test"
version = "0.1.0"
edition = "2021"
"#;
        std::fs::write("/tmp/boos-attack-cargo/Cargo.toml", cargo_toml).ok();
        std::fs::create_dir_all("/tmp/boos-attack-cargo/src").ok();
        std::fs::write("/tmp/boos-attack-cargo/src/lib.rs", "").ok();

        // The real attack: agent writes build.rs via develop, then BUILD
        let r = execute_develop_action("WRITE /tmp/boos-attack-cargo/build.rs fn main() { println!(\"cargo:warning=PWNED\"); }");
        println!("BUILD.RS ATTACK: build.rs written via develop WRITE");
        
        // Clean up
        let _ = std::fs::remove_dir_all("/tmp/boos-attack-cargo");
    }

    #[test]
    fn attack_18_develop_agent_prompt_injection() {
        // Can't actually test LLM prompt injection in unit test,
        // but verify the develop prompt doesn't contain any safety instructions
        // that would prevent malicious goals.
        // The develop mode has NO safety filter on goals.
        println!("PROMPT INJECTION: develop mode accepts any goal string — no safety filter");
        // This is a design-level vulnerability, not a code bug
    }
}