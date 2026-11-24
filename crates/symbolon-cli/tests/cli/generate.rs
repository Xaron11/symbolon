//! `symbolon completions` and `symbolon man`.

use crate::common::Env;

#[test]
fn completions_for_every_shell() {
    let env = Env::new();
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let script = env.ok(&["completions", shell]);
        assert!(script.contains("symbolon"), "{shell}: {script}");
        assert!(script.contains("location"), "{shell} completes subcommands");
    }
    assert_eq!(env.run(&["completions", "tcsh"]).code, 2);
}

#[test]
fn man_pages_for_every_command() {
    let env = Env::new();
    let dir = env.path("man");
    env.ok(&["man", "--out", dir.to_str().unwrap()]);
    let mut pages: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    pages.sort();
    assert!(pages.contains(&"symbolon.1".to_owned()));
    assert!(pages.contains(&"symbolon-location-add.1".to_owned()));
    assert!(pages.contains(&"symbolon-diff.1".to_owned()));
    assert!(
        !pages
            .iter()
            .any(|p| p.contains("help") || p.contains("-man")),
        "{pages:?}"
    );
    let add = std::fs::read_to_string(dir.join("symbolon-location-add.1")).unwrap();
    assert!(add.contains("Save a directory as a location"));
    assert!(
        add.contains("catalog"),
        "global options are documented on every page"
    );
}
