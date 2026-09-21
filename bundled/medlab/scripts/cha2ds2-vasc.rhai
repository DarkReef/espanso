// CHA2DS2-VASc и CHA2DS2-VA. Контракт reactive preview unstable.

fn point(value) { if value == "Да" { 1 } else { 0 } }
fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_integer(value) { let text = value; text.trim(); parse_int(text) }

fn calculate(input) {
    let age = 0;
    try { age = parse_integer(input.age); } catch { return fail("Возраст должен быть целым числом."); }
    if age < 18 || age > 120 { return fail("Проверьте возраст пациента."); }

    let age_points = if age >= 75 { 2 } else if age >= 65 { 1 } else { 0 };
    let base = point(input.heart_failure) + point(input.hypertension) + age_points + point(input.diabetes);
    base += 2 * point(input.stroke_tia) + point(input.vascular_disease);
    let score_va = base;
    let score_vasc = base + if input.sex == "Женщина" { 1 } else { 0 };

    let recommendation = if score_va >= 2 {
        "антикоагуляция рекомендована при отсутствии противопоказаний"
    } else if score_va == 1 {
        "антикоагуляцию следует рассмотреть с учётом клинического контекста"
    } else {
        "низкий балльный риск; требуется периодический пересмотр"
    };
    let text = `CHA₂DS₂-VASc: ${score_vasc} баллов; CHA₂DS₂-VA: ${score_va} баллов (${recommendation}).`;
    #{ status: "ok", value: score_va, score_va: score_va, score_vasc: score_vasc, text: text }
}
