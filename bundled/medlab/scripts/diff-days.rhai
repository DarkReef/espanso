// Включительный расчёт календарных дней между датами dd.mm.yyyy.
// Поддерживает обычный string-input и reactive form map.

fn fail(message) { #{ status: "error", value: "", text: message } }
fn is_leap(year) { year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) }
fn days_in_month(year, month) {
    if month == 2 { if is_leap(year) { 29 } else { 28 } }
    else if month == 4 || month == 6 || month == 9 || month == 11 { 30 }
    else { 31 }
}

fn date_serial(value) {
    let normalized = value;
    normalized.trim();
    let parts = normalized.split(".");
    if parts.len != 3 { return -1; }

    let day = 0;
    let month = 0;
    let year = 0;
    try { day = parse_int(parts[0]); } catch { return -1; }
    try { month = parse_int(parts[1]); } catch { return -1; }
    try { year = parse_int(parts[2]); } catch { return -1; }
    if year < 1900 || year > 2200 || month < 1 || month > 12 { return -1; }
    if day < 1 || day > days_in_month(year, month) { return -1; }

    let previous_year = year - 1;
    let serial = previous_year * 365 + previous_year / 4 - previous_year / 100 + previous_year / 400;
    let current_month = 1;
    while current_month < month {
        serial += days_in_month(year, current_month);
        current_month += 1;
    }
    serial + day
}

fn range_map(from_value, to_value) {
    let from_day = date_serial(from_value);
    let to_day = date_serial(to_value);
    if from_day < 0 || to_day < 0 { return fail("Введите даты в формате ДД.ММ.ГГГГ."); }
    if to_day < from_day { return fail("Конечная дата не может быть раньше начальной."); }
    let days = to_day - from_day + 1;
    #{ status: "ok", value: days, text: `ЧДН: ${days} дней (включительно)` }
}

fn calculate(input) {
    let parts = input.split("|");
    if parts.len != 2 { return "0"; }
    let result = range_map(parts[0], parts[1]);
    if result.status == "ok" { `${result.value}` } else { "0" }
}

fn calculate_form(input) { range_map(input.from, input.to) }
fn calculate_primary(input) { range_map(input.primary_date, input.today) }
