use super::*;
use crate::transport::{How, run};

fn reason(failure: Failure) -> String {
    match failure {
        Failure::Stop(payload) => payload["reason"].as_str().unwrap_or_default().to_owned(),
        other => panic!("expected a named refusal, got {}", other.detail()),
    }
}

fn malformed(text: &str) {
    let failure = parse(text).expect_err("a declaration that cannot run as written was read");
    assert_eq!(reason(failure), "checks-declaration-malformed", "{text}");
}

#[test]
fn a_valid_declaration_is_read_in_order() {
    let declared = parse(
        "[[check]]\nname = \"unit-tests\"\ncommand = [\"cargo\", \"test\"]\n\n\
         [[check]]\nname = \"lint2\"\ncwd = \"web\"\ncommand = [\"npm\", \"run\", \"lint\"]\n",
    )
    .expect("a valid declaration");
    assert_eq!(
        declared,
        vec![
            Check {
                name: "unit-tests".to_owned(),
                command: vec!["cargo".to_owned(), "test".to_owned()],
                cwd: None,
            },
            Check {
                name: "lint2".to_owned(),
                command: vec!["npm".to_owned(), "run".to_owned(), "lint".to_owned()],
                cwd: Some("web".to_owned()),
            },
        ]
    );
}

#[test]
fn a_file_without_check_tables_declares_none() {
    assert_eq!(parse("").expect("empty"), Vec::new());
    assert_eq!(
        parse("[[generated]]\npath = \"a\"\ncommand = [\"x\"]\n").expect("generated only"),
        Vec::new()
    );
}

#[test]
fn each_malformed_declaration_is_refused_and_none_reads_as_empty() {
    malformed("[[check]]\nname = \"a\"\ncommand = [\"x\"]\nextra = 1\n");
    malformed(
        "[[check]]\nname = \"a\"\ncommand = [\"x\"]\n[[check]]\nname = \"a\"\ncommand = [\"y\"]\n",
    );
    malformed("[[check]]\nname = \"a\"\ncommand = []\n");
    malformed("[[check]]\nname = \"a\"\ncommand = [\"\"]\n");
    malformed("[[check]]\nname = \"a\"\ncommand = [\"x\", 3]\n");
    malformed("[[check]]\nname = \"a\"\n");
    malformed("[[check]]\ncommand = [\"x\"]\n");
    malformed("[[check]]\nname = \"a\"\ncwd = \"../up\"\ncommand = [\"x\"]\n");
    malformed("[[check]]\nname = \"a\"\ncwd = \"/abs\"\ncommand = [\"x\"]\n");
    malformed("[[check]]\nname = \"Upper\"\ncommand = [\"x\"]\n");
    malformed("[[check]]\nname = \"a--b\"\ncommand = [\"x\"]\n");
    malformed("[[check]]\nname = \"-a\"\ncommand = [\"x\"]\n");
    malformed("[[check]]\nname = \"a_b\"\ncommand = [\"x\"]\n");
    malformed("check = 3\n");
    malformed("[[check]\nname = ");
}

#[test]
fn the_commands_digest_follows_order_and_every_field() {
    let one = Check {
        name: "a".to_owned(),
        command: vec!["x".to_owned(), "y".to_owned()],
        cwd: None,
    };
    let two = Check {
        name: "b".to_owned(),
        command: vec!["z".to_owned()],
        cwd: Some("sub".to_owned()),
    };
    let both = commands_digest(&[one.clone(), two.clone()]);
    assert_eq!(both, commands_digest(&[one.clone(), two.clone()]));
    assert_ne!(both, commands_digest(&[two.clone(), one.clone()]));
    assert_ne!(
        commands_digest(std::slice::from_ref(&one)),
        commands_digest(&[Check {
            command: vec!["xy".to_owned()],
            ..one.clone()
        }])
    );
    assert_ne!(
        commands_digest(std::slice::from_ref(&one)),
        commands_digest(&[Check {
            command: vec!["x".to_owned(), "y".to_owned(), String::new()],
            ..one.clone()
        }])
    );
    assert_ne!(
        commands_digest(std::slice::from_ref(&two)),
        commands_digest(&[Check { cwd: None, ..two }])
    );
}

fn git(at: &Path, arguments: &[&str]) -> String {
    let mut command = vec!["git"];
    command.extend_from_slice(arguments);
    run(&command, Some(at), How::write())
        .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()))
        .stdout
}

#[test]
fn only_the_base_commit_declares_checks() {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path();
    git(repo, &["init", "--quiet", "-b", "main"]);
    git(repo, &["config", "user.email", "estigia@example.invalid"]);
    git(repo, &["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join("a.txt"), "a\n").expect("content");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "--quiet", "-m", "base"]);
    let base = git(repo, &["rev-parse", "HEAD"]).trim().to_owned();

    std::fs::write(
        repo.join("estigia.generated.toml"),
        "[[check]]\nname = \"sneaky\"\ncommand = [\"true\"]\n",
    )
    .expect("a declaration in the head");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "--quiet", "-m", "head"]);

    assert_eq!(
        declared_at(repo, &base).expect("the base is read"),
        Vec::new()
    );
    let head = git(repo, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(declared_at(repo, &head).expect("the head is read").len(), 1);
}

#[test]
fn a_generated_declaration_still_reads_beside_check_tables() {
    let text = "[[generated]]\npath = \"a\"\ncommand = [\"x\"]\n\n[[check]]\nname = \"c\"\ncommand = [\"y\"]\n";
    assert_eq!(
        crate::transport::generated::parse(text)
            .expect("generated beside check")
            .len(),
        1
    );
}
