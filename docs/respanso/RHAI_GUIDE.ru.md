# Rhai в rEspanso

Rhai используется для локальных вычислений и преобразования текста без Python, PowerShell и Node.js.

## Где хранить скрипты

Пользовательские файлы размещаются в `scripts/**/*.rhai`. Match Studio рекурсивно показывает их, компилирует и добавляет ошибки в диагностику проекта.

Подключение из YAML:

```yaml
vars:
  - name: result
    type: rhai
    params:
      path: "%CONFIG%/scripts/example.rhai"
      function: calculate
      input: source
```

`path` обязателен. `function` можно не указывать: по умолчанию вызывается `calculate`.

## Главный контракт

Каждый рабочий скрипт должен объявлять вызываемую функцию:

```rhai
fn calculate(input) {
    `Получено: ${input}`
}
```

Последнее выражение внутри функции становится её результатом. Последнее выражение верхнего уровня файла не используется как результат rEspanso.

## Что находится в `input`

Значение зависит от YAML-параметра `input`:

- если `input` указывает на строковую переменную, Rhai получает строку;
- если `input` указывает на форму или другую multi-value переменную, Rhai получает map;
- если `input` не указан, Rhai получает map всех доступных переменных текущего правила.

Пример со строкой:

```rhai
fn calculate(input) {
    let value = input.trim();
    if value == "" {
        "Данные не указаны"
    } else {
        `Получено: ${value}`
    }
}
```

Пример с формой:

```rhai
fn calculate(input) {
    let age = parse_int(input.age);
    `Через год: ${age + 1}`
}
```

Все поля формы передаются строками.

## Возврат строки и map

Строка подходит для простого результата:

```rhai
fn calculate(input) {
    input.trim().to_upper()
}
```

Map удобен, когда нужны основной результат и подробный текст:

```rhai
fn calculate(input) {
    let value = parse_int(input.value);
    #{
        status: "ok",
        value: value * 2,
        text: `Удвоенное значение: ${value * 2}`
    }
}
```

Обычная Rhai-переменная возвращает поля map как подполя:

```yaml
replace: |
  Число: {{result.value}}
  {{result.text}}
```

В реактивной форме computed-поля после отправки получают префикс узла и разделитель `__`; подробности приведены в [REACTIVE_RHAI_PREVIEW.ru.md](REACTIVE_RHAI_PREVIEW.ru.md).

## Безопасный разбор чисел

```rhai
fn parse_number(value) {
    let normalized = value.trim();
    normalized.replace(",", ".");
    parse_float(normalized)
}
```

До разбора проверяйте обязательные поля, если пустая строка допустима в форме:

```rhai
fn calculate(input) {
    if input.value.trim() == "" {
        return #{ status: "error", value: "", text: "Укажите значение." };
    }

    let value = parse_float(input.value);
    #{ status: "ok", value: value, text: `Значение: ${value}` }
}
```

## Реактивный предпросмотр

Для live/manual/submit-предпросмотра Rhai подключается внутри `params.computed` формы:

```yaml
matches:
  - trigger: ":double"
    replace: "{{double_form.result__text}}"
    vars:
      - name: double_form
        type: form
        params:
          preview: true
          preview_mode: live
          preview_layout: "{{result.text}}"
          layout: "Число: [[value]]"
          fields:
            value:
              type: text
              default: "21"
          computed:
            result:
              type: rhai
              path: "%CONFIG%/scripts/double.rhai"
              function: calculate
              depends_on: []
```

Не добавляйте второй `type: rhai` после формы: используйте computed-результат напрямую.

## Ограничения встроенного движка

- `eval` и `import` отключены;
- shell, сеть, файловая система и внешние процессы недоступны;
- путь должен находиться внутри `%CONFIG%` или `%PACKAGES%`;
- вычисление ограничено по операциям, глубине, размеру строк, массивов и map;
- `round(value)` и `value.round()` доступны для чисел с плавающей точкой.

## Практические рекомендации

- делайте функции детерминированными;
- проверяйте пустые и граничные значения;
- возвращайте `status: "error"` с понятным `text` для ошибок формы;
- не смешивайте расчёт и внешние побочные эффекты;
- не помещайте персональные данные в демонстрационные тесты;
- после изменения дождитесь фоновой проверки Match Studio.
