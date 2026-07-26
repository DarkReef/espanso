// rEspanso embedded Rhai calculator example.
//
// This file is interpreted at runtime by rEspanso. Editing it does not require
// Rust, Cargo, Node.js, Python, PowerShell modules or a new application build.
// It is only a UI/runtime demonstration and not a validated clinical score.

fn calculate(input) {
    let age = parse_int(input.age);
    let category_points = if input.category == "Категория 2" { 2 } else { 0 };
    let factor_points = if input.factor_a == "Да" { 1 } else { 0 };
    let demo_score = age / 10 + category_points + factor_points;

    `Демонстрационный результат: ${demo_score}\n\nВозраст: ${age}\nКатегория: ${input.category}\nФактор A: ${input.factor_a}\n\nЭто визуальный пример, не клиническая шкала.`
}
