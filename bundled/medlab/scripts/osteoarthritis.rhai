// Формулировка диагноза остеоартрита. Контракт reactive preview unstable.

fn clean(value) { let text = value; text.trim(); text }
fn fail(message) { #{ status: "error", value: "", text: message } }

fn localization_text(localization, side, involved_groups, other_location) {
    if localization == "Коленный сустав" {
        if side == "Правый" { "Остеоартрит правого коленного сустава" }
        else if side == "Левый" { "Остеоартрит левого коленного сустава" }
        else if side == "Двусторонний" { "Двусторонний остеоартрит коленных суставов" }
        else { "Остеоартрит коленных суставов" }
    } else if localization == "Тазобедренный сустав" {
        if side == "Правый" { "Остеоартрит правого тазобедренного сустава" }
        else if side == "Левый" { "Остеоартрит левого тазобедренного сустава" }
        else if side == "Двусторонний" { "Двусторонний остеоартрит тазобедренных суставов" }
        else { "Остеоартрит тазобедренных суставов" }
    } else if localization == "Суставы кистей" {
        if side == "Правый" { "Остеоартрит суставов правой кисти" }
        else if side == "Левый" { "Остеоартрит суставов левой кисти" }
        else if side == "Двусторонний" { "Двусторонний остеоартрит суставов кистей" }
        else { "Остеоартрит суставов кистей" }
    } else if localization == "Суставы позвоночника" {
        "Остеоартрит суставов позвоночника"
    } else if localization == "Первый плюснефаланговый сустав" {
        if side == "Правый" { "Остеоартрит первого плюснефалангового сустава правой стопы" }
        else if side == "Левый" { "Остеоартрит первого плюснефалангового сустава левой стопы" }
        else if side == "Двусторонний" { "Двусторонний остеоартрит первых плюснефаланговых суставов" }
        else { "Остеоартрит первого плюснефалангового сустава" }
    } else if localization == "Генерализованный остеоартрит" {
        `Генерализованный остеоартрит с вовлечением: ${involved_groups}`
    } else {
        let custom = clean(other_location);
        if custom == "" { "Остеоартрит, локализация требует уточнения" } else { `Остеоартрит: ${custom}` }
    }
}

fn calculate(input) {
    let groups = clean(input.involved_groups);
    if input.localization == "Генерализованный остеоартрит" && groups == "" {
        return fail("Для генерализованного остеоартрита укажите не менее 3 групп суставов.");
    }
    if input.localization == "Другая локализация" && clean(input.other_location) == "" {
        return fail("Укажите другую локализацию остеоартрита.");
    }

    let diagnosis = localization_text(input.localization, input.side, groups, input.other_location);
    if input.etiology == "Первичный" { diagnosis += ", первичный"; }
    else if input.etiology == "Вторичный" { diagnosis += ", вторичный"; }

    if input.stage == "Требует уточнения" { diagnosis += ", рентгенологическая стадия требует уточнения"; }
    else { diagnosis += `, рентгенологическая стадия ${input.stage} по Kellgren–Lawrence`; }

    let complications = clean(input.complications);
    if complications != "" && complications != "Нет" && complications != "Без синовита" { diagnosis += `. ${complications}`; }
    let deformity = clean(input.deformity);
    if deformity != "" && deformity != "Нет" { diagnosis += `. Деформация: ${deformity}`; }
    if input.functional_class == "Требует уточнения" { diagnosis += ". ФК требует уточнения"; }
    else { diagnosis += `. ФК ${input.functional_class}`; }
    diagnosis += ".";

    #{ status: "ok", value: diagnosis, text: diagnosis }
}
