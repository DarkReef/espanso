// HAS-BLED score for bleeding-risk factor review in atrial fibrillation.
// Definitions follow ESC AF guidelines (2010/2020); current-use note follows ESC 2024.
// Module version: 1.0.0 (2026-07-26).

fn point(value) {
    if value == "Да" { 1 } else { 0 }
}

fn calculate(input) {
    let age = parse_int(input.age);
    if age < 18 || age > 120 {
        return `HAS-BLED не рассчитан.\n\nПроверьте возраст пациента.`;
    }

    let hypertension = point(input.hypertension);
    let renal = point(input.renal);
    let liver = point(input.liver);
    let stroke = point(input.stroke);
    let bleeding = point(input.bleeding);
    let labile_inr = point(input.labile_inr);
    let elderly = if age > 65 { 1 } else { 0 };
    let drugs = point(input.drugs);
    let alcohol = point(input.alcohol);

    let score = hypertension + renal + liver + stroke + bleeding + labile_inr + elderly + drugs + alcohol;
    let interpretation = if score >= 3 {
        "Высокий риск по исходному порогу: требуется осторожность, коррекция модифицируемых факторов и регулярный пересмотр"
    } else {
        "Порог высокого риска (≥3) не достигнут; факторы всё равно следует регулярно пересматривать"
    };

    let components = "";
    if hypertension == 1 { components += "H — САД >160 мм рт. ст.;\n"; }
    if renal == 1 { components += "A — нарушение функции почек;\n"; }
    if liver == 1 { components += "A — нарушение функции печени;\n"; }
    if stroke == 1 { components += "S — инсульт в анамнезе;\n"; }
    if bleeding == 1 { components += "B — кровотечение/предрасположенность;\n"; }
    if labile_inr == 1 { components += "L — лабильное МНО/TTR <60%;\n"; }
    if elderly == 1 { components += "E — возраст >65 лет;\n"; }
    if drugs == 1 { components += "D — антиагрегант или НПВП;\n"; }
    if alcohol == 1 { components += "D — чрезмерное употребление алкоголя;\n"; }
    if components == "" { components = "Активных компонентов не отмечено.\n"; }

    `HAS-BLED: ${score} из 9\n${interpretation}\n\nУчтённые компоненты:\n${components}\nСовременные рекомендации: не использовать балл как самостоятельную причину не назначать или отменять антикоагулянт; исправлять модифицируемые факторы риска кровотечения.\n\nМодуль v1.0.0. Результат поддерживает, но не заменяет клиническое решение.`
}
