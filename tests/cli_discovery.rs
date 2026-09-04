#![cfg(feature = "cli")]

use assert_cmd::Command;

fn help(arguments: &[&str]) -> String {
    let output = Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).expect("UTF-8 help")
}

#[test]
fn help_lists_every_public_command_family() {
    let output = help(&["--help"]);
    for family in [
        "analyze", "classical", "nearest-space", "project", "batch", "prepost",
        "profile-plan", "simulate", "multimodal", "smoke", "cohort", "bayes",
        "longitudinal", "spatial3d", "causal", "numerics", "policy", "graph",
        "topology", "registration", "neural", "backend",
    ] {
        assert!(
            output.lines().any(|line| line.split_whitespace().next() == Some(family)),
            "missing family {family} in root help:\n{output}"
        );
    }
}

#[test]
fn family_help_includes_commands_from_every_existing_parser() {
    for (family, commands) in [
        ("project", &["classical", "cohort-energy", "hierarchical-normal", "marked-prepost"][..]),
        ("bayes", &["normal-mean", "hierarchical-normal", "ordinal-group", "hmc-normal", "arbitrary-window-ipp-likelihood", "dirichlet-multinomial-group-sbc"][..]),
        ("multimodal", &["analyze", "pcca", "spatial-latent-factor"][..]),
        ("spatial3d", &["serial-stack", "validate-advanced"][..]),
        ("causal", &["observational", "active-design"][..]),
    ] {
        let output = help(&[family, "--help"]);
        for command in commands {
            assert!(
                output.lines().any(|line| line.split_whitespace().next() == Some(command)),
                "missing {family} {command} in family help:\n{output}"
            );
        }
    }
}

#[test]
fn help_subcommand_and_flag_resolve_the_same_nested_commands() {
    for path in [
        &["cohort"][..],
        &["project", "hierarchical-normal"][..],
        &["bayes", "ordinal-group"][..],
        &["bayes", "dirichlet-multinomial-group-sbc"][..],
        &["multimodal", "pcca"][..],
    ] {
        let mut flag = path.to_vec();
        flag.push("--help");
        let mut subcommand = vec!["help"];
        subcommand.extend_from_slice(path);
        assert_eq!(help(&flag), help(&subcommand), "help path {path:?}");
    }
}

#[test]
fn nested_argument_errors_retain_the_owned_schema() {
    for path in [
        &["project", "hierarchical-normal"][..],
        &["bayes", "ordinal-group"][..],
        &["cohort", "permutation"][..],
    ] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args(path)
            .arg("--unsupported-option")
            .assert()
            .code(2)
            .stderr(predicates::str::contains("--unsupported-option"));
    }
}
