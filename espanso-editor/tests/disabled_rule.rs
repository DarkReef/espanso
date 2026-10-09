use espanso_editor::workspace::MatchWorkspace;
use std::fs;
use tempdir::TempDir;

#[test]
fn disabled_trigger_is_serialized_and_excluded_from_matching() {
    let temp = TempDir::new("respanso-disabled-trigger").expect("temp directory");
    let match_dir = temp.path().join("match");
    fs::create_dir_all(&match_dir).expect("match directory");
    let file = match_dir.join("base.yml");
    fs::write(
        &file,
        "matches:\n  - trigger: \":disabled_test\"\n    replace: \"visible\"\n",
    )
    .expect("fixture");

    let mut workspace = MatchWorkspace::load(temp.path()).expect("workspace");
    assert_eq!(workspace.playground("prefix :disabled_test").len(), 1);

    let id = workspace.rules().remove(0).id;
    let mut draft = workspace.rule(&id).expect("rule").draft;
    draft.disabled = true;
    workspace.update_rule(&id, &draft).expect("disable rule");

    assert!(workspace.playground("prefix :disabled_test").is_empty());
    assert!(workspace
        .raw_file(&file)
        .expect("raw file")
        .contains("disabled: true"));

    workspace.save_all().expect("save disabled rule");
    let reloaded = MatchWorkspace::load(temp.path()).expect("reload");
    let reloaded_rule = reloaded.rules().remove(0);
    assert!(reloaded_rule.draft.disabled);
    assert!(reloaded.playground("prefix :disabled_test").is_empty());
}
