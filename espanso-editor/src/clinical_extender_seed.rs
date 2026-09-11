impl ClinicalExtender {
    /// Loads the user library and adds non-destructive starter templates that are
    /// missing from it. Existing templates and investigation labels are never
    /// overwritten by this seed migration.
    pub fn load_seeded(root: PathBuf) -> Self {
        let mut editor = Self::load(root);
        let added = ensure_common_clinical_seed(&mut editor.db);
        if added > 0 {
            editor.status = format!(
                "Добавлено стартовых элементов клинической библиотеки: {added}. Проверьте и сохраните библиотеку."
            );
            editor.recompose(false);
        }
        editor
    }
}

fn ensure_common_clinical_seed(db: &mut ClinicalDatabase) -> usize {
    let mut added = 0_usize;

    let investigations = [
        ("lipid_profile", "Липидный профиль"),
        ("creatinine_egfr", "Креатинин с расчётом рСКФ"),
        ("potassium", "Калий"),
        ("sodium", "Натрий"),
        ("tsh", "ТТГ"),
        ("free_t4", "Свободный Т4"),
        ("hba1c", "HbA1c"),
        ("urine_acr", "Альбумин/креатинин мочи (АКМ/МАУ)"),
        ("holter", "Холтеровское мониторирование ЭКГ"),
        ("spirometry", "Спирометрия"),
        ("bronchodilator_test", "Бронходилатационный тест"),
        ("spo2", "SpO₂"),
        ("chest_xray_if_indicated", "Рентгенография ОГК (по показаниям)"),
        ("egd_if_indicated", "ФГДС (по показаниям)"),
        ("h_pylori", "Исследование на Helicobacter pylori"),
        ("ferritin", "Ферритин"),
        ("transferrin_saturation", "Насыщение трансферрина железом"),
        ("crp", "С-реактивный белок"),
        ("alt_ast", "АЛТ, АСТ"),
        ("stress_test_if_indicated", "Нагрузочное тестирование / визуализация ишемии (по показаниям)"),
        ("spine_imaging_if_indicated", "Визуализация позвоночника (по показаниям)"),
    ];
    for (id, label) in investigations {
        if ensure_seed_investigation(db, id, label) {
            added += 1;
        }
    }

    let templates = vec![
        seed_template(
            "I10",
            "Эссенциальная (первичная) гипертензия",
            &["АГ", "ГИПЕРТОНИЯ"],
            "Артериальная гипертензия диагностирована ранее. Длительность заболевания: [уточнить]. Постоянная антигипертензивная терапия: [уточнить]. Домашний контроль АД: [уточнить].",
            &["echocardiography", "urine_acr", "creatinine_egfr", "potassium", "lipid_profile"],
        ),
        seed_template(
            "I48.*",
            "Фибрилляция и трепетание предсердий",
            &["ФП", "ФИБРИЛЛЯЦИЯ ПРЕДСЕРДИЙ", "ТП"],
            "ФП/ТП: форма и длительность аритмии [уточнить]. Симптомность и частота эпизодов [уточнить]. Антикоагулянтная и пульсурежающая/антиаритмическая терапия [уточнить].",
            &["echocardiography", "holter", "tsh", "potassium", "creatinine_egfr"],
        ),
        seed_template(
            "I25.*",
            "Хроническая ишемическая болезнь сердца",
            &["ИБС", "ХИБС"],
            "ИБС: клиническая форма [уточнить]. Перенесённые ОКС и/или реваскуляризация [уточнить]. Антиагрегантная и гиполипидемическая терапия [уточнить].",
            &["lipid_profile", "alt_ast", "echocardiography", "stress_test_if_indicated"],
        ),
        seed_template(
            "E11.*",
            "Сахарный диабет 2 типа",
            &["СД2", "СД 2", "ДИАБЕТ 2"],
            "Сахарный диабет 2 типа: длительность [уточнить]. Сахароснижающая терапия [уточнить]. Самоконтроль гликемии [уточнить]. Наличие известных микрососудистых и макрососудистых осложнений [уточнить].",
            &["hba1c", "creatinine_egfr", "urine_acr", "lipid_profile", "alt_ast"],
        ),
        seed_template(
            "N18.*",
            "Хроническая болезнь почек",
            &["ХБП", "CKD"],
            "ХБП: стадия и категория альбуминурии [уточнить]. Исходный уровень креатинина/рСКФ и динамика [уточнить]. Нефротоксичные препараты и эпизоды ОПП [уточнить].",
            &["creatinine_egfr", "potassium", "sodium", "urine_acr", "cbc"],
        ),
        seed_template(
            "J44.*",
            "Хроническая обструктивная болезнь лёгких",
            &["ХОБЛ", "COPD"],
            "ХОБЛ: стаж курения/экспозиция [уточнить]. Частота обострений и госпитализаций за последний год [уточнить]. Ингаляционная терапия и техника ингаляции [уточнить].",
            &["spirometry", "spo2", "cbc", "chest_xray_if_indicated"],
        ),
        seed_template(
            "J45.*",
            "Бронхиальная астма",
            &["БА", "АСТМА"],
            "Бронхиальная астма: возраст дебюта и триггеры [уточнить]. Частота дневных/ночных симптомов и потребность в препарате облегчения [уточнить]. Базисная терапия и техника ингаляции [уточнить].",
            &["spirometry", "bronchodilator_test", "spo2", "cbc"],
        ),
        seed_template(
            "K21.*",
            "Гастроэзофагеальная рефлюксная болезнь",
            &["ГЭРБ", "GERD"],
            "ГЭРБ: длительность симптомов, связь с приёмом пищи и положением тела [уточнить]. Наличие дисфагии, кровотечения, похудания и иных симптомов тревоги [уточнить]. Текущая антисекреторная терапия [уточнить].",
            &["egd_if_indicated"],
        ),
        seed_template(
            "K25.*",
            "Язва желудка",
            &["ЯБЖ", "ЯЗВА ЖЕЛУДКА"],
            "Язвенная болезнь/язва желудка: дата и способ верификации [уточнить]. Наличие кровотечения или иных осложнений [уточнить]. Приём НПВП/антиагрегантов/антикоагулянтов [уточнить].",
            &["egd_if_indicated", "h_pylori", "cbc"],
        ),
        seed_template(
            "K26.*",
            "Язва двенадцатиперстной кишки",
            &["ЯБДПК", "ЯЗВА ДПК"],
            "Язвенная болезнь/язва ДПК: дата и способ верификации [уточнить]. Наличие кровотечения или иных осложнений [уточнить]. Приём НПВП/антиагрегантов/антикоагулянтов [уточнить].",
            &["egd_if_indicated", "h_pylori", "cbc"],
        ),
        seed_template(
            "E78.*",
            "Нарушения обмена липопротеинов и другие липидемии",
            &["ДИСЛИПИДЕМИЯ", "ГХС"],
            "Дислипидемия: известный исходный и целевой уровень липидов [уточнить]. Гиполипидемическая терапия, переносимость и приверженность [уточнить].",
            &["lipid_profile", "alt_ast"],
        ),
        seed_template(
            "E03.*",
            "Другие формы гипотиреоза",
            &["ГИПОТИРЕОЗ"],
            "Гипотиреоз: этиология и длительность [уточнить]. Заместительная терапия левотироксином и режим приёма [уточнить]. Последний контроль ТТГ [уточнить].",
            &["tsh", "free_t4"],
        ),
        seed_template(
            "D50.*",
            "Железодефицитная анемия",
            &["ЖДА", "ЖЕЛЕЗОДЕФИЦИТ"],
            "Железодефицитная анемия: впервые выявлена/известна ранее [уточнить]. Источник кровопотери или другая причина дефицита железа [уточнить]. Препараты железа и ответ на терапию [уточнить].",
            &["cbc", "ferritin", "transferrin_saturation", "crp"],
        ),
        seed_template(
            "M54.*",
            "Дорсалгия",
            &["ДОРСАЛГИЯ", "БОЛЬ В СПИНЕ"],
            "Боль в спине: локализация, длительность и связь с нагрузкой [уточнить]. Неврологические симптомы и признаки тревоги [уточнить]. Предшествующая анальгетическая терапия и её эффект [уточнить].",
            &["spine_imaging_if_indicated"],
        ),
    ];

    for template in templates {
        if ensure_seed_template(db, template) {
            added += 1;
        }
    }

    added
}

fn ensure_seed_investigation(db: &mut ClinicalDatabase, id: &str, label: &str) -> bool {
    if db.catalog.iter().any(|item| item.id == id) {
        return false;
    }
    db.catalog.push(Investigation {
        id: id.to_owned(),
        label: label.to_owned(),
    });
    true
}

fn ensure_seed_template(db: &mut ClinicalDatabase, template: NosologyTemplate) -> bool {
    if db.templates.iter().any(|existing| {
        existing
            .code_pattern
            .trim()
            .eq_ignore_ascii_case(template.code_pattern.trim())
    }) {
        return false;
    }
    db.templates.push(template);
    true
}

fn seed_template(
    code_pattern: &str,
    title: &str,
    aliases: &[&str],
    disease_history: &str,
    investigation_ids: &[&str],
) -> NosologyTemplate {
    NosologyTemplate {
        is_base: false,
        code_pattern: code_pattern.to_owned(),
        title: title.to_owned(),
        aliases: aliases.iter().map(|value| (*value).to_owned()).collect(),
        sections: ClinicalSections {
            disease_history: disease_history.to_owned(),
            ..Default::default()
        },
        investigation_ids: investigation_ids
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

#[cfg(test)]
mod seed_tests {
    use super::*;

    #[test]
    fn seed_is_idempotent_and_does_not_overwrite_existing_templates() {
        let mut db = ClinicalDatabase::default();
        let first = ensure_common_clinical_seed(&mut db);
        assert!(first > 0);
        let count_after_first = db.templates.len();
        let second = ensure_common_clinical_seed(&mut db);
        assert_eq!(second, 0);
        assert_eq!(db.templates.len(), count_after_first);
    }

    #[test]
    fn common_aliases_compose_expected_investigations() {
        let mut db = ClinicalDatabase::default();
        ensure_common_clinical_seed(&mut db);
        let tokens = parse_diagnosis_input("ФП, СД2, ХБП");
        let (document, _, unknown) = compose(&db, &tokens);
        assert!(unknown.is_empty());
        assert!(document.examination_plan.contains("Холтеровское"));
        assert!(document.examination_plan.contains("HbA1c"));
        assert!(document.examination_plan.contains("рСКФ"));
    }
}
