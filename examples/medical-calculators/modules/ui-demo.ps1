$age = $env:ESPANSO_CALCULATOR_INPUTS_AGE
$category = $env:ESPANSO_CALCULATOR_INPUTS_CATEGORY
$factorA = $env:ESPANSO_CALCULATOR_INPUTS_FACTOR_A

if ([string]::IsNullOrWhiteSpace($age)) {
    Write-Error "Поле 'Возраст' не передано модулю."
    exit 10
}

@"
Внешний модуль выполнен успешно.

Возраст: $age
Категория: $category
Фактор A: $factorA

Клиническая формула в ядро rEspanso не встроена.
Этот модуль только демонстрирует визуальную форму и внешний расчётный контур.
"@
