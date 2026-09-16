#!/usr/bin/env python3
from pathlib import Path

CORE = Path("espanso-editor/src/clinical_extender_core.rs")
FORM = Path("espanso-modulo/src/sys/form/form.cpp")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"{label}: anchor not found")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, new: str, label: str) -> str:
    left = text.find(start)
    if left < 0:
        raise SystemExit(f"{label}: start anchor not found")
    right = text.find(end, left + len(start))
    if right < 0:
        raise SystemExit(f"{label}: end anchor not found")
    return text[:left] + new + text[right:]


t = CORE.read_text(encoding="utf-8")

t = replace_once(
    t,
    'use serde::{Deserialize, Serialize};',
    'use serde::{de::DeserializeOwned, Deserialize, Serialize};',
    'serde import',
)

t = replace_once(
    t,
    'const STORE_DIR: &str = "clinical_extender";\nconst STORE_FILE: &str = "nosologies.yml";\nconst DATABASE_VERSION: u32 = 2;',
    'const STORE_DIR: &str = "clinical_extender";\nconst STORE_META_FILE: &str = "catalog.yml";\nconst STORE_BASE_FILE: &str = "base.yml";\nconst STORE_NOSOLOGIES_DIR: &str = "nosologies";\nconst LEGACY_STORE_FILE: &str = "nosologies.yml";\nconst DATABASE_VERSION: u32 = 3;',
    'store constants',
)

store_meta = r'''

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
struct ClinicalStoreMeta {
    version: u32,
    catalog: Vec<Investigation>,
    fields: Vec<ClinicalFieldDefinition>,
}

impl Default for ClinicalStoreMeta {
    fn default() -> Self {
        Self {
            version: DATABASE_VERSION,
            catalog: Vec::new(),
            fields: Vec::new(),
        }
    }
}
'''
anchor = '''pub struct ClinicalDatabase {
    pub version: u32,
    pub catalog: Vec<Investigation>,
    pub templates: Vec<NosologyTemplate>,
    pub fields: Vec<ClinicalFieldDefinition>,
}
'''
t = replace_once(t, anchor, anchor + store_meta, 'store metadata model')

old_load = r'''    pub fn load(root: PathBuf) -> Self {
        let path = database_path(&root);
        let (mut db, status) = match load_database(&path) {
            Ok(Some(db)) => (db, format!("Библиотека загружена: {}", path.display())),
            Ok(None) => (
                ClinicalDatabase::default(),
                "Создана стартовая локальная библиотека нозологий".to_owned(),
            ),
            Err(error) => (
                ClinicalDatabase::default(),
                format!("Не удалось прочитать библиотеку; загружены стартовые шаблоны: {error}"),
            ),
        };'''
new_load = r'''    pub fn load(root: PathBuf) -> Self {
        let store = store_root(&root);
        let had_split_store = split_store_exists(&store);
        let had_legacy_store = legacy_database_path(&root).exists();
        let (mut db, status) = match load_database(&store) {
            Ok(Some(db)) if had_split_store => (
                db,
                format!("Библиотека загружена из отдельных файлов: {}", store.display()),
            ),
            Ok(Some(db)) if had_legacy_store => (
                db,
                "Загружен старый nosologies.yml; при сохранении библиотека будет разложена по отдельным файлам"
                    .to_owned(),
            ),
            Ok(Some(db)) => (db, format!("Библиотека загружена: {}", store.display())),
            Ok(None) => (
                ClinicalDatabase::default(),
                "Создана стартовая локальная библиотека нозологий".to_owned(),
            ),
            Err(error) => (
                ClinicalDatabase::default(),
                format!("Не удалось прочитать библиотеку; загружены стартовые шаблоны: {error}"),
            ),
        };'''
t = replace_once(t, old_load, new_load, 'ClinicalExtender::load')

start = '        ui.group(|ui| {\n            ui.label(egui::RichText::new("Структурированный план обследования").strong());'
end = '\n\n        template_text_field('
new_investigations = r'''        let base_ids = base_investigation_ids(&self.db);
        let catalog = self.db.catalog.clone();
        ui.group(|ui| {
            if self.draft.is_base {
                ui.label(egui::RichText::new("Базовый набор обследований").strong());
                ui.small("Этот набор автоматически наследуется всеми нозологиями.");
            } else {
                ui.label(egui::RichText::new("Наследуется из базового профиля").strong());
                if base_ids.is_empty() {
                    ui.label(egui::RichText::new("Базовый набор пуст").weak());
                } else {
                    for id in &base_ids {
                        ui.label(format!("• {}", investigation_label(&self.db, id)));
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("Собственные исследования нозологии").strong());
                ui.small("Здесь хранятся только дополнения к базовому набору.");
            }

            let own_ids = self.draft.investigation_ids.clone();
            if own_ids.is_empty() {
                ui.label(egui::RichText::new("Собственных исследований нет").weak());
            } else {
                for id in own_ids {
                    let label = investigation_label(&self.db, &id);
                    ui.horizontal(|ui| {
                        ui.label(format!("• {label}"));
                        if ui.small_button("Убрать").clicked() {
                            self.draft.investigation_ids.retain(|value| value != &id);
                            self.draft_dirty = true;
                        }
                    });
                }
            }

            let inherited = if self.draft.is_base {
                Vec::new()
            } else {
                base_ids.clone()
            };
            let available = catalog
                .iter()
                .filter(|investigation| {
                    !self.draft.investigation_ids.contains(&investigation.id)
                        && !inherited.contains(&investigation.id)
                })
                .cloned()
                .collect::<Vec<_>>();
            ui.menu_button("Добавить исследование", |ui| {
                if available.is_empty() {
                    ui.label(egui::RichText::new("Все доступные исследования уже подключены").weak());
                }
                for investigation in &available {
                    if ui.button(&investigation.label).clicked() {
                        self.draft.investigation_ids.push(investigation.id.clone());
                        self.draft_dirty = true;
                        ui.close();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_investigation)
                        .hint_text("Новое исследование в общий каталог"),
                );
                if ui.button("Добавить").clicked() {
                    self.add_custom_investigation();
                }
            });
        });'''
t = replace_between(t, start, end, new_investigations, 'investigation editor')

add_start = '    fn add_custom_investigation(&mut self) {'
add_end = '\n\n    fn save_database(&mut self) {'
new_add = r'''    fn add_custom_investigation(&mut self) {
        let label = self.new_investigation.trim().to_owned();
        if label.is_empty() {
            return;
        }
        let inherited = if self.draft.is_base {
            Vec::new()
        } else {
            base_investigation_ids(&self.db)
        };
        if let Some(existing) = self
            .db
            .catalog
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case(&label))
            .cloned()
        {
            if inherited.contains(&existing.id) {
                self.status = format!("{} уже входит в базовый набор и наследуется автоматически", existing.label);
                self.new_investigation.clear();
                return;
            }
            if !self.draft.investigation_ids.contains(&existing.id) {
                self.draft.investigation_ids.push(existing.id);
                self.draft_dirty = true;
            }
            self.new_investigation.clear();
            return;
        }
        let mut counter = self.db.catalog.len() + 1;
        let id = loop {
            let candidate = format!("custom_{counter}");
            if self.db.catalog.iter().all(|item| item.id != candidate) {
                break candidate;
            }
            counter += 1;
        };
        self.db.catalog.push(Investigation {
            id: id.clone(),
            label,
        });
        self.draft.investigation_ids.push(id);
        self.new_investigation.clear();
        self.draft_dirty = true;
    }'''
t = replace_between(t, add_start, add_end, new_add, 'custom investigation')

old_save_method = r'''    fn save_database(&mut self) {
        ensure_base_template(&mut self.db);
        match save_database(&database_path(&self.root), &self.db) {
            Ok(()) => {
                self.saved_db = serialize_database(&self.db).unwrap_or_default();
                self.draft_dirty = false;
                self.status = format!(
                    "Библиотека сохранена: {}",
                    database_path(&self.root).display()
                );
            }
            Err(error) => self.status = format!("Не удалось сохранить библиотеку: {error}"),
        }
    }'''
new_save_method = r'''    fn save_database(&mut self) {
        ensure_base_template(&mut self.db);
        let store = store_root(&self.root);
        match save_database(&store, &self.db) {
            Ok(()) => {
                self.saved_db = serialize_database(&self.db).unwrap_or_default();
                self.draft_dirty = false;
                self.status = format!(
                    "Библиотека сохранена по отдельным файлам: {}",
                    store.display()
                );
            }
            Err(error) => self.status = format!("Не удалось сохранить библиотеку: {error}"),
        }
    }'''
t = replace_once(t, old_save_method, new_save_method, 'save_database method')

persist_start = 'fn database_path(root: &Path) -> PathBuf {'
persist_end = 'fn ensure_base_template(db: &mut ClinicalDatabase) {'
persistence = r'''fn store_root(root: &Path) -> PathBuf {
    root.join(STORE_DIR)
}

fn legacy_database_path(root: &Path) -> PathBuf {
    store_root(root).join(LEGACY_STORE_FILE)
}

fn split_store_exists(store: &Path) -> bool {
    store.join(STORE_META_FILE).is_file() && store.join(STORE_BASE_FILE).is_file()
}

fn read_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    serde_norway::from_str(&content)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn write_yaml<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let content = serde_norway::to_string(value).map_err(|error| error.to_string())?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, content).map_err(|error| format!("{}: {error}", temp.display()))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    fs::rename(&temp, path).map_err(|error| format!("{}: {error}", path.display()))
}

fn load_database(store: &Path) -> Result<Option<ClinicalDatabase>, String> {
    if split_store_exists(store) {
        let meta: ClinicalStoreMeta = read_yaml(&store.join(STORE_META_FILE))?;
        let mut base: NosologyTemplate = read_yaml(&store.join(STORE_BASE_FILE))?;
        base.is_base = true;
        if base.code_pattern.trim().is_empty() {
            base.code_pattern = "BASE".to_owned();
        }

        let mut templates = vec![base];
        let nosologies_dir = store.join(STORE_NOSOLOGIES_DIR);
        if nosologies_dir.is_dir() {
            let mut files = fs::read_dir(&nosologies_dir)
                .map_err(|error| format!("{}: {error}", nosologies_dir.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"))
                })
                .collect::<Vec<_>>();
            files.sort();
            for path in files {
                let mut template: NosologyTemplate = read_yaml(&path)?;
                template.is_base = false;
                templates.push(template);
            }
        }
        return Ok(Some(ClinicalDatabase {
            version: meta.version.max(DATABASE_VERSION),
            catalog: meta.catalog,
            templates,
            fields: meta.fields,
        }));
    }

    let legacy = store.join(LEGACY_STORE_FILE);
    if legacy.is_file() {
        return read_yaml(&legacy).map(Some);
    }
    Ok(None)
}

fn serialize_database(db: &ClinicalDatabase) -> Result<String, String> {
    serde_norway::to_string(db).map_err(|error| error.to_string())
}

fn sanitize_nosology_filename(code: &str) -> String {
    let mut result = String::new();
    for ch in code.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            result.push(ch);
        } else if ch == '*' {
            result.push_str("_star");
        } else {
            result.push('_');
        }
    }
    let result = result.trim_matches('_');
    if result.is_empty() {
        "nosology".to_owned()
    } else {
        result.to_owned()
    }
}

fn save_database(store: &Path, db: &ClinicalDatabase) -> Result<(), String> {
    fs::create_dir_all(store).map_err(|error| format!("{}: {error}", store.display()))?;
    let nosologies_dir = store.join(STORE_NOSOLOGIES_DIR);
    fs::create_dir_all(&nosologies_dir)
        .map_err(|error| format!("{}: {error}", nosologies_dir.display()))?;

    let meta = ClinicalStoreMeta {
        version: DATABASE_VERSION,
        catalog: db.catalog.clone(),
        fields: db.fields.clone(),
    };
    write_yaml(&store.join(STORE_META_FILE), &meta)?;

    let base = db
        .templates
        .iter()
        .find(|template| template.is_base)
        .ok_or_else(|| "В библиотеке отсутствует базовый профиль".to_owned())?;
    write_yaml(&store.join(STORE_BASE_FILE), base)?;

    let mut used_names = HashSet::new();
    for (index, template) in db.templates.iter().filter(|template| !template.is_base).enumerate() {
        let stem = sanitize_nosology_filename(&template.code_pattern);
        let mut filename = format!("{stem}.yml");
        let mut suffix = 2;
        while !used_names.insert(filename.clone()) {
            filename = format!("{stem}_{suffix}.yml");
            suffix += 1;
        }
        if template.code_pattern.trim().is_empty() {
            filename = format!("nosology_{}.yml", index + 1);
            used_names.insert(filename.clone());
        }
        write_yaml(&nosologies_dir.join(&filename), template)?;
    }

    for entry in fs::read_dir(&nosologies_dir)
        .map_err(|error| format!("{}: {error}", nosologies_dir.display()))?
    {
        let path = entry.map_err(|error| error.to_string())?.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"));
        let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        if is_yaml && !used_names.contains(name) {
            fs::remove_file(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

fn investigation_label(db: &ClinicalDatabase, id: &str) -> String {
    db.catalog
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.label.clone())
        .unwrap_or_else(|| id.to_owned())
}

fn base_investigation_ids(db: &ClinicalDatabase) -> Vec<String> {
    db.templates
        .iter()
        .find(|template| template.is_base)
        .map(|template| template.investigation_ids.clone())
        .unwrap_or_default()
}

'''
t = replace_between(t, persist_start, persist_end, persistence, 'split persistence')

# Extend tests without disturbing existing tests.
insert = r'''

    #[test]
    fn split_store_roundtrip_keeps_nosologies_separate() {
        let temp = tempdir::TempDir::new("clinical_split_store").unwrap();
        let store = store_root(temp.path());
        let db = ClinicalDatabase::default();
        save_database(&store, &db).unwrap();

        assert!(store.join(STORE_META_FILE).is_file());
        assert!(store.join(STORE_BASE_FILE).is_file());
        assert!(store.join(STORE_NOSOLOGIES_DIR).join("I11.9.yml").is_file());
        assert!(store.join(STORE_NOSOLOGIES_DIR).join("I50.9.yml").is_file());

        let loaded = load_database(&store).unwrap().unwrap();
        assert_eq!(loaded.catalog, db.catalog);
        assert_eq!(loaded.templates.len(), db.templates.len());
    }

    #[test]
    fn legacy_store_migrates_on_save_without_deleting_backup() {
        let temp = tempdir::TempDir::new("clinical_legacy_store").unwrap();
        let store = store_root(temp.path());
        fs::create_dir_all(&store).unwrap();
        let db = ClinicalDatabase::default();
        fs::write(store.join(LEGACY_STORE_FILE), serialize_database(&db).unwrap()).unwrap();

        let loaded = load_database(&store).unwrap().unwrap();
        assert_eq!(loaded.templates.len(), db.templates.len());
        save_database(&store, &loaded).unwrap();
        assert!(store.join(LEGACY_STORE_FILE).is_file());
        assert!(store.join(STORE_BASE_FILE).is_file());
        assert!(store.join(STORE_NOSOLOGIES_DIR).join("I11.9.yml").is_file());
    }

    #[test]
    fn disease_inherits_base_investigations_without_duplicates() {
        let db = ClinicalDatabase::default();
        let tokens = parse_diagnosis_input("I11.9");
        let (document, _, _) = compose(&db, &tokens);
        assert_eq!(document.examination_plan.matches("ЭКГ").count(), 1);
        assert!(document.examination_plan.contains("ЭхоКГ"));
        assert!(document.examination_plan.contains("МАУ"));
    }
'''
pos = t.rfind('\n}')
if pos < 0:
    raise SystemExit('tests module end not found')
t = t[:pos] + insert + t[pos:]

CORE.write_text(t, encoding="utf-8")

# Restore compact, resizable wx form. Keep the vertical scroll from the prior fix.
f = FORM.read_text(encoding="utf-8")
f = replace_once(
    f,
    'const long DEFAULT_STYLE = wxSTAY_ON_TOP | wxCLOSE_BOX | wxCAPTION;',
    'const long DEFAULT_STYLE = wxSTAY_ON_TOP | wxCLOSE_BOX | wxCAPTION | wxRESIZE_BORDER | wxMAXIMIZE_BOX;',
    'form resizable style',
)
f = replace_once(
    f,
    '''bool FormApp::OnInit() {
    const wxSize &maxFormSize =
        wxSize(formMetadata->maxWindowWidth, formMetadata->maxWindowHeight);
    FormFrame *frame =
        new FormFrame(wxString::FromUTF8(formMetadata->windowTitle),
                      wxPoint(50, 50), maxFormSize);''',
    '''bool FormApp::OnInit() {
    const wxSize initialFormSize = wxSize(610, 320);
    FormFrame *frame =
        new FormFrame(wxString::FromUTF8(formMetadata->windowTitle),
                      wxPoint(50, 50), initialFormSize);''',
    'form initial size',
)
f = replace_once(
    f,
    '''    panel = new wxScrolledWindow(this, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxVSCROLL);
    panel->SetScrollRate(0, 10);
    wxBoxSizer *vbox = new wxBoxSizer(wxVERTICAL);
    panel->SetSizer(vbox);''',
    '''    panel = new wxScrolledWindow(this, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxVSCROLL);
    panel->SetScrollRate(0, 10);
    wxBoxSizer *frameSizer = new wxBoxSizer(wxVERTICAL);
    frameSizer->Add(panel, 1, wxEXPAND);
    this->SetSizer(frameSizer);
    wxBoxSizer *vbox = new wxBoxSizer(wxVERTICAL);
    panel->SetSizer(vbox);''',
    'frame resize sizer',
)
old_fit = '''void FormFrame::FitFormToContent() {
    const wxSize best = panel->GetBestSize();
    const int maxWidth = std::max(320, formMetadata->maxWindowWidth);
    const int maxHeight = std::max(180, formMetadata->maxWindowHeight);
    const int width = std::min(std::max(best.GetWidth(), 320), maxWidth);
    const int height = std::min(std::max(best.GetHeight(), 180), maxHeight);

    panel->SetVirtualSize(best);
    panel->FitInside();
    this->SetClientSize(wxSize(width, height));
    this->Layout();
}'''
new_fit = '''void FormFrame::FitFormToContent() {
    const wxSize best = panel->GetBestSize();
    panel->SetVirtualSize(best);
    panel->FitInside();
    this->SetMinClientSize(wxSize(420, 240));
    this->SetClientSize(wxSize(610, 320));
    this->Layout();
}'''
f = replace_once(f, old_fit, new_fit, 'form fit behavior')
FORM.write_text(f, encoding="utf-8")

checks = {
    CORE: [
        'const STORE_META_FILE: &str = "catalog.yml";',
        'const STORE_BASE_FILE: &str = "base.yml";',
        'const STORE_NOSOLOGIES_DIR: &str = "nosologies";',
        'Наследуется из базового профиля',
        'Собственные исследования нозологии',
        'split_store_roundtrip_keeps_nosologies_separate',
    ],
    FORM: [
        'wxRESIZE_BORDER',
        'wxSize(610, 320)',
        'frameSizer->Add(panel, 1, wxEXPAND)',
        'wxScrolledWindow',
    ],
}
for path, markers in checks.items():
    content = path.read_text(encoding="utf-8")
    for marker in markers:
        if marker not in content:
            raise SystemExit(f"verification failed: {marker!r} missing from {path}")

print("clinical split-store, inheritance UI, and resizable form patch applied")
