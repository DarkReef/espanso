// The legacy nosologies.yml remains untouched during migration. New saves use
// base.yml plus one YAML file per nosology, with a previous-directory backup.
#[derive(Serialize, Deserialize)]
struct ClinicalBaseFile {
    version: u32,
    #[serde(default)]
    catalog: Vec<Investigation>,
    #[serde(default)]
    fields: Vec<ClinicalFieldDefinition>,
    template: NosologyTemplate,
    #[serde(default)]
    order: Vec<String>,
}

fn read_clinical_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_norway::from_str(&content).map_err(|e| format!("{}: {e}", path.display()))
}

fn nosology_filename(template: &NosologyTemplate) -> String {
    let mut name = String::from("nosology-");
    for byte in template.code_pattern.trim().to_uppercase().bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'.' {
            name.push(char::from(byte));
        } else {
            name.push_str(&format!("_{byte:02x}"));
        }
    }
    name.push_str(".yml");
    name
}

fn validate_clinical_database(db: &ClinicalDatabase) -> Result<(), String> {
    if db.version > DATABASE_VERSION {
        return Err(format!("Версия библиотеки {} новее поддерживаемой", db.version));
    }
    if db.templates.iter().filter(|t| t.is_base).count() != 1 {
        return Err("Библиотека должна содержать ровно один базовый профиль".to_owned());
    }
    let mut names = HashSet::new();
    for template in db.templates.iter().filter(|t| !t.is_base) {
        if template.code_pattern.trim().is_empty() || !names.insert(nosology_filename(template)) {
            return Err(format!("Пустой или повторяющийся код нозологии: {}", template.code_pattern));
        }
    }
    Ok(())
}

fn load_database(path: &Path) -> Result<Option<ClinicalDatabase>, String> {
    let directory = path.parent().ok_or("Некорректный путь библиотеки")?;
    let backup = directory.with_extension("previous");
    // Recover an interrupted directory swap before loading or allowing a save.
    if !directory.exists() && backup.exists() {
        fs::rename(&backup, directory).map_err(|e| format!("Восстановление библиотеки: {e}"))?;
    }
    let base_path = directory.join("base.yml");
    if !base_path.exists() {
        if directory.join("nosologies").exists() {
            return Err(format!("Отсутствует {}", base_path.display()));
        }
        if !path.exists() {
            return Ok(None);
        }
        let mut db: ClinicalDatabase = read_clinical_yaml(path)?;
        if db.version > DATABASE_VERSION {
            return Err("Неподдерживаемая версия библиотеки".to_owned());
        }
        ensure_base_template(&mut db);
        // Older versions collected every encountered field in a global list.
        // Move used definitions to their templates to avoid leaking diagnosis-
        // specific fields into all visits after migration.
        let legacy_fields = std::mem::take(&mut db.fields);
        for field in legacy_fields {
            let mut assigned = false;
            for template in &mut db.templates {
                if section_field_names(&template.sections).contains(&field.name) {
                    if !template.fields.iter().any(|f| f.name == field.name) {
                        template.fields.push(field.clone());
                    }
                    assigned = true;
                }
            }
            if !assigned {
                db.fields.push(field);
            }
        }
        validate_clinical_database(&db)?;
        return Ok(Some(db));
    }
    let base: ClinicalBaseFile = read_clinical_yaml(&base_path)?;
    if !base.template.is_base {
        return Err(format!("{}: ожидается базовый профиль", base_path.display()));
    }
    let mut db = ClinicalDatabase {
        version: base.version, catalog: base.catalog, fields: base.fields,
        templates: vec![base.template],
    };
    let folder = directory.join("nosologies");
    let mut paths = Vec::new();
    // The directory is required, even for an empty library, so accidental
    // deletion cannot silently turn into a successful empty load and save.
    for entry in fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if matches!(path.extension().and_then(|s| s.to_str()), Some("yml" | "yaml")) {
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                return Err(format!("Ожидается обычный YAML-файл: {}", path.display()));
            }
            paths.push(path);
        }
    }
    paths.sort();
    paths.sort_by_key(|p| base.order.iter().position(|name| Some(name.as_str()) == p.file_name().and_then(|s| s.to_str())).unwrap_or(usize::MAX));
    for path in paths {
        let template: NosologyTemplate = read_clinical_yaml(&path)?;
        if template.is_base {
            return Err(format!("{}: базовый профиль должен быть в base.yml", path.display()));
        }
        db.templates.push(template);
    }
    validate_clinical_database(&db)?;
    Ok(Some(db))
}

fn serialize_database(db: &ClinicalDatabase) -> Result<String, String> {
    serde_norway::to_string(db).map_err(|error| error.to_string())
}

fn copy_clinical_directory(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            copy_clinical_directory(&entry.path(), &destination)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination).map_err(|e| e.to_string())?;
        } else {
            return Err(format!("Ссылки в библиотеке не поддерживаются: {}", entry.path().display()));
        }
    }
    Ok(())
}

fn save_database(path: &Path, db: &ClinicalDatabase) -> Result<(), String> {
    validate_clinical_database(db)?;
    let directory = path.parent().ok_or("Некорректный путь библиотеки")?;
    // Validate existing files before any writes. Never replace a damaged library
    // with the starter templates loaded for display after an error.
    load_database(path)?;
    let pending = directory.with_extension("pending");
    let backup = directory.with_extension("previous");
    if pending.exists() {
        fs::remove_dir_all(&pending).map_err(|e| e.to_string())?;
    }
    if directory.exists() {
        copy_clinical_directory(directory, &pending)?;
    } else {
        fs::create_dir_all(&pending).map_err(|e| e.to_string())?;
    }
    let folder = pending.join("nosologies");
    if folder.exists() {
        // Only replace YAML files managed by this library, retaining notes etc.
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if matches!(path.extension().and_then(|s| s.to_str()), Some("yml" | "yaml")) {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
    }
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut order = Vec::new();
    for template in db.templates.iter().filter(|t| !t.is_base) {
        let filename = nosology_filename(template);
        let content = serde_norway::to_string(template).map_err(|e| e.to_string())?;
        fs::write(folder.join(&filename), content).map_err(|e| e.to_string())?;
        order.push(filename);
    }
    let base = ClinicalBaseFile {
        version: DATABASE_VERSION, catalog: db.catalog.clone(), fields: db.fields.clone(),
        template: db.templates.iter().find(|t| t.is_base).ok_or("Нет базового профиля")?.clone(),
        order,
    };
    let content = serde_norway::to_string(&base).map_err(|e| e.to_string())?;
    fs::write(pending.join("base.yml"), content).map_err(|e| e.to_string())?;
    load_database(&pending.join(STORE_FILE))?.ok_or("Проверка сохранённой библиотеки не пройдена")?;
    if directory.exists() {
        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|e| e.to_string())?;
        }
        fs::rename(directory, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(&pending, directory) {
        if backup.exists() {
            fs::rename(&backup, directory).map_err(|restore| format!("{error}; восстановление: {restore}. Резервная копия: {}", backup.display()))?;
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn legacy_field_definitions_follow_their_nosology() {
        let dir = tempdir::TempDir::new("clinical-fields").unwrap();
        let path = database_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut db = ClinicalDatabase::default();
        db.templates[1].sections.complaints = "{{local_field}}".into();
        db.fields.push(ClinicalFieldDefinition {
            name: "local_field".into(), kind: ClinicalFieldKind::Integer, choices: vec![],
        });
        fs::write(&path, serialize_database(&db).unwrap()).unwrap();
        let loaded = load_database(&path).unwrap().unwrap();
        assert!(effective_fields(&loaded, &[]).is_empty());
        assert_eq!(effective_fields(&loaded, &["I11.9".into()])[0].kind, ClinicalFieldKind::Integer);
        save_database(&path, &loaded).unwrap();
        assert_eq!(load_database(&path).unwrap().unwrap(), loaded);
    }

    #[test]
    fn migrates_legacy_without_changing_original_and_roundtrips_scoped_data() {
        let dir = tempdir::TempDir::new("clinical-split").unwrap();
        let path = database_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut db = ClinicalDatabase::default();
        db.templates[1].fields.push(ClinicalFieldDefinition {
            name: "severity".into(), kind: ClinicalFieldKind::Choice,
            choices: vec!["one".into(), "two".into()],
        });
        db.templates[1].catalog.push(investigation("custom_test", "Local test"));
        db.templates[1].investigation_ids.push("custom_test".into());
        let legacy = serialize_database(&db).unwrap();
        fs::write(&path, &legacy).unwrap();
        let loaded = load_database(&path).unwrap().unwrap();
        save_database(&path, &loaded).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
        assert!(path.parent().unwrap().join("base.yml").is_file());
        assert_eq!(fs::read_dir(path.parent().unwrap().join("nosologies")).unwrap().count(), 3);
        assert_eq!(load_database(&path).unwrap().unwrap(), db);
    }

    #[test]
    fn deletion_and_rename_do_not_leave_old_templates() {
        let dir = tempdir::TempDir::new("clinical-delete").unwrap();
        let path = database_path(dir.path());
        let mut db = ClinicalDatabase::default();
        save_database(&path, &db).unwrap();
        db.templates.remove(1);
        db.templates[1].code_pattern = "I50.*".into();
        save_database(&path, &db).unwrap();
        assert_eq!(load_database(&path).unwrap().unwrap(), db);
        let editor = ClinicalExtender::load_seeded(dir.path().to_owned());
        assert_eq!(editor.db, db);
    }

    #[test]
    fn invalid_yaml_blocks_load_and_save_without_overwriting_files() {
        let dir = tempdir::TempDir::new("clinical-invalid").unwrap();
        let path = database_path(dir.path());
        let db = ClinicalDatabase::default();
        save_database(&path, &db).unwrap();
        let invalid = path.parent().unwrap().join("nosologies/broken.yml");
        fs::write(&invalid, "title: [broken").unwrap();
        assert!(load_database(&path).unwrap_err().contains("broken.yml"));
        let before = fs::read(path.parent().unwrap().join("base.yml")).unwrap();
        assert!(save_database(&path, &db).is_err());
        assert_eq!(fs::read_to_string(invalid).unwrap(), "title: [broken");
        assert_eq!(fs::read(path.parent().unwrap().join("base.yml")).unwrap(), before);
    }

    #[test]
    fn interrupted_directory_swap_recovers_previous_library() {
        let dir = tempdir::TempDir::new("clinical-recover").unwrap();
        let path = database_path(dir.path());
        let db = ClinicalDatabase::default();
        save_database(&path, &db).unwrap();
        let folder = path.parent().unwrap();
        fs::rename(folder, folder.with_extension("previous")).unwrap();
        assert_eq!(load_database(&path).unwrap().unwrap(), db);
        assert!(folder.join("base.yml").is_file());
    }

    #[test]
    fn duplicate_codes_are_rejected_and_filenames_cannot_escape_directory() {
        let mut db = ClinicalDatabase::default();
        db.templates.push(db.templates[1].clone());
        assert!(validate_clinical_database(&db).is_err());
        let template = NosologyTemplate { code_pattern: "../../CON:*\\test".into(), ..Default::default() };
        let filename = nosology_filename(&template);
        assert!(!filename.contains('/') && !filename.contains('\\') && !filename.contains(':'));
        assert_eq!(Path::new(&filename).components().count(), 1);
    }

    #[test]
    fn fields_override_base_only_for_selected_nosology() {
        let mut db = ClinicalDatabase::default();
        db.templates[0].fields.push(ClinicalFieldDefinition {
            name: "level".into(), kind: ClinicalFieldKind::Text, choices: vec![],
        });
        db.templates[1].fields.push(ClinicalFieldDefinition {
            name: "level".into(), kind: ClinicalFieldKind::Integer, choices: vec![],
        });
        assert_eq!(effective_fields(&db, &[])[0].kind, ClinicalFieldKind::Text);
        assert_eq!(effective_fields(&db, &["I11.9".into()])[0].kind, ClinicalFieldKind::Integer);
        assert_eq!(effective_fields(&db, &["I50.9".into()])[0].kind, ClinicalFieldKind::Text);
    }

    #[test]
    fn own_investigations_extend_base_without_duplicates_or_leaking() {
        let mut db = ClinicalDatabase::default();
        db.templates[1].catalog.push(investigation("special", "Special investigation"));
        db.templates[1].investigation_ids.extend(["special".into(), "cbc".into()]);
        let (document, _, _) = compose(&db, &["I11.9".into()]);
        assert!(document.examination_plan.contains("Special investigation"));
        assert_eq!(document.examination_plan.matches("ОАК").count(), 1);
        let (other, _, _) = compose(&db, &["I50.9".into()]);
        assert!(!other.examination_plan.contains("Special investigation"));
    }
}
