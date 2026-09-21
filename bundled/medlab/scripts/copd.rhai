// ХОБЛ: GOLD, mMRC/CAT и группа A/B/E. Контракт reactive preview unstable.

fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_number(value) { let text = value; text.trim(); text.replace(",", "."); parse_float(text) }
fn parse_integer(value) { let text = value; text.trim(); parse_int(text) }

fn calculate(input) {
    let ratio = 0.0;
    let fev1 = 0.0;
    let cat = 0;
    let moderate = 0;
    let hospital = 0;
    try { ratio = parse_number(input.fev1_fvc); } catch { return fail("ОФВ₁/ФЖЕЛ должно быть числом."); }
    try { fev1 = parse_number(input.fev1_percent); } catch { return fail("ОФВ₁ должен быть числом."); }
    try { cat = parse_integer(input.cat_score); } catch { return fail("CAT должен быть целым числом."); }
    try { moderate = parse_integer(input.moderate_exacerbations); } catch { return fail("Число обострений должно быть целым."); }
    try { hospital = parse_integer(input.hospitalizations); } catch { return fail("Число госпитализаций должно быть целым."); }
    if ratio <= 0.0 || ratio > 1.5 || fev1 <= 0.0 || fev1 > 200.0 || cat < 0 || cat > 40 || moderate < 0 || hospital < 0 {
        return fail("Проверьте спирометрию, CAT и количество обострений.");
    }
    if ratio >= 0.70 {
        return #{ status: "ok", value: "ХОБЛ не подтверждена", text: "ХОБЛ не подтверждена: постбронходилатационный ОФВ₁/ФЖЕЛ ≥0,70. Требуется клиническая и спирометрическая переоценка." };
    }

    let gold = if fev1 >= 80.0 { "GOLD 1" } else if fev1 >= 50.0 { "GOLD 2" } else if fev1 >= 30.0 { "GOLD 3" } else { "GOLD 4" };
    let mmrc = if input.mmrc.starts_with("0") { 0 } else if input.mmrc.starts_with("1") { 1 } else if input.mmrc.starts_with("2") { 2 } else if input.mmrc.starts_with("3") { 3 } else { 4 };
    let high_symptoms = mmrc >= 2 || cat >= 10;
    let group = if moderate >= 2 || hospital >= 1 { "E" } else if high_symptoms { "B" } else { "A" };
    let dn = if input.respiratory_failure == "Не указывать" { "" } else { `; ${input.respiratory_failure}` };
    let text = `Хроническая обструктивная болезнь лёгких: бронхиальная обструкция ${gold} (ОФВ₁ ${fev1}% от должного), группа ${group}; mMRC ${mmrc}, CAT ${cat} баллов; за 12 месяцев: среднетяжёлых обострений ${moderate}, госпитализаций ${hospital}${dn}.`;
    #{ status: "ok", value: group, gold: gold, group: group, text: text }
}
