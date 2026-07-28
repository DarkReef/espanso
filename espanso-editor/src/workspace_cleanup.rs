use crate::{workspace::MatchWorkspace, yaml_imports};
use std::fs;

impl Drop for MatchWorkspace {
    fn drop(&mut self) {
        if !self.dirty_files().is_empty() {
            return;
        }

        let files = self.files();
        let missing_files = files
            .iter()
            .filter(|path| !path.exists())
            .cloned()
            .collect::<Vec<_>>();
        if missing_files.is_empty() {
            return;
        }

        let Some(base_file) = yaml_imports::find_base_file(&files, self.match_root()) else {
            return;
        };
        let Ok(mut base_content) = fs::read_to_string(&base_file) else {
            return;
        };

        for missing_file in missing_files {
            let Ok(updated) =
                yaml_imports::update_import(&base_content, &base_file, &missing_file, false)
            else {
                continue;
            };
            base_content = updated;
        }

        let Ok(current_content) = fs::read_to_string(&base_file) else {
            return;
        };
        if base_content != current_content {
            let _ = fs::write(base_file, base_content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[test]
    fn removes_deleted_file_from_base_imports() {
        let temp = TempDir::new("respanso-import-cleanup").expect("temp dir");
        let match_dir = temp.path().join("match");
        fs::create_dir_all(&match_dir).expect("match dir");
        let base = match_dir.join("base.yml");
        let extra = match_dir.join("extra.yml");
        fs::write(
            &base,
            "imports:\n  - extra.yml\n\nmatches:\n  - trigger: :base\n    replace: Base\n",
        )
        .expect("write base");
        fs::write(
            &extra,
            "matches:\n  - trigger: :extra\n    replace: Extra\n",
        )
        .expect("write extra");

        let workspace = MatchWorkspace::load(temp.path()).expect("workspace");
        fs::remove_file(&extra).expect("remove extra");
        drop(workspace);

        let content = fs::read_to_string(base).expect("read base");
        assert!(!content.contains("extra.yml"));
    }

    #[test]
    fn leaves_imports_untouched_while_workspace_is_dirty() {
        let temp = TempDir::new("respanso-import-cleanup-dirty").expect("temp dir");
        let match_dir = temp.path().join("match");
        fs::create_dir_all(&match_dir).expect("match dir");
        let base = match_dir.join("base.yml");
        let extra = match_dir.join("extra.yml");
        fs::write(
            &base,
            "imports:\n  - extra.yml\n\nmatches:\n  - trigger: :base\n    replace: Base\n",
        )
        .expect("write base");
        fs::write(
            &extra,
            "matches:\n  - trigger: :extra\n    replace: Extra\n",
        )
        .expect("write extra");

        let mut workspace = MatchWorkspace::load(temp.path()).expect("workspace");
        let id = crate::workspace::RuleId {
            file: base.clone(),
            ordinal: 0,
        };
        let mut draft = workspace.rule(&id).expect("rule").draft;
        draft.replace = "Changed".to_owned();
        workspace.update_rule(&id, &draft).expect("update");
        fs::remove_file(&extra).expect("remove extra");
        drop(workspace);

        let content = fs::read_to_string(base).expect("read base");
        assert!(content.contains("extra.yml"));
    }
}
