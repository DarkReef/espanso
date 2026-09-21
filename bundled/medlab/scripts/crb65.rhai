// CRB-65: тяжесть внебольничной пневмонии. Контракт reactive preview unstable.

fn point(value) { if value == "Да" { 1 } else { 0 } }
fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_integer(value) { let text = value; text.trim(); parse_int(text) }

fn calculate(input) {
    let age = 0;
    try { age = parse_integer(input.age); } catch { return fail("Возраст должен быть целым числом."); }
    if age < 18 || age > 120 { return fail("Проверьте возраст пациента."); }

    let score = point(input.confusion) + point(input.respiratory_rate) + point(input.low_blood_pressure);
    if age >= 65 { score += 1; }

    let recommendation = if score == 0 {
        "Низкий риск. Возможно амбулаторное лечение"
    } else if score <= 2 {
        "Умеренный риск. Рекомендуется оценка возможности госпитализации, особенно при 2 баллах"
    } else {
        "Высокий риск. Показана экстренная госпитализация; при 3–4 баллах следует рассмотреть лечение в отделении интенсивной терапии"
    };
    #{ status: "ok", value: score, text: `CRB-65: ${score} баллов. ${recommendation}.` }
}
