// HAS-BLED для комплексной формы ФП. Контракт reactive preview unstable.

fn point(value) { if value == "Да" { 1 } else { 0 } }
fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_integer(value) { let text = value; text.trim(); parse_int(text) }

fn calculate(input) {
    let age = 0;
    try { age = parse_integer(input.age); } catch { return fail("Возраст должен быть целым числом."); }
    if age < 18 || age > 120 { return fail("Проверьте возраст пациента."); }

    let score = point(input.hasbled_hypertension) + point(input.renal_dysfunction) + point(input.liver_dysfunction);
    score += point(input.stroke_history) + point(input.bleeding_history) + point(input.labile_inr);
    score += point(input.drugs) + point(input.alcohol);
    if age > 65 { score += 1; }

    let interpretation = if score >= 3 { "высокий риск; необходимы коррекция факторов и частое наблюдение" } else { "порог высокого риска не достигнут" };
    let text = `HAS-BLED: ${score} баллов (${interpretation})`;
    #{ status: "ok", value: score, text: text }
}
