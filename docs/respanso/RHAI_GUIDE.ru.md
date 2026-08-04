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
    let value = input;
    value.trim();
    if value == "" {
        "Данные не указаны"
    } else {
        `Получено: ${value}`
    }
}
```

Пример с формой:

```rhai
fn parse_integer(value) {
    let normalized = value;
    normalized.trim();
    parse_int(normalized)
}

fn calculate(input) {
    let age = 0;
    try {
        age = parse_integer(input.age);
    } catch {
        return #{
            status: "error",
            value: "",
            text: "Возраст должен быть целым числом."
        };
    }

    #{
        status: "ok",
        value: age + 1,
        text: `Через год: ${age + 1}`
    }
}
```

Все поля формы передаются строками.

## Возврат строки и map

Строка подходит для простого результата:

```rhai
fn calculate(input) {
    let value = input;
    value.trim();
    value.to_upper()
}
```

Map удобен, когда нужны основной результат и подробный текст:

```rhai
fn parse_integer(value) {
    let normalized = value;
    normalized.trim();
    parse_int(normalized)
}

fn calculate(input) {
    let value = 0;
    try {
        value = parse_integer(input.value);
    } catch {
        return #{
            status: "error",
            value: "",
            text: "Значение должно быть целым числом."
        };
    }

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
    let normalized = value;
    normalized.trim();
    normalized.replace(",", ".");
    parse_float(normalized)
}

fn parse_integer(value) {
    let normalized = value;
    normalized.trim();
    parse_int(normalized)
}
```

Строковые методы текущего runtime применяйте к локальной копии. Не передавайте результат `trim()` напрямую в `parse_int` или `parse_float`.

До разбора проверяйте обязательные поля, а ошибку преобразования перехватывайте через statement-форму `try/catch`:

```rhai
fn calculate(input) {
    if input.value.trim() == "" {
        return #{ status: "error", value: "", text: "Укажите значение." };
    }

    let value = 0.0;
    try {
        value = parse_number(input.value);
    } catch {
        return #{
            status: "error",
            value: "",
            text: "Значение должно быть числом."
        };
    }

    #{ status: "ok", value: value, text: `Значение: ${value}` }
}
```

Текущий встроенный Rhai не поддерживает конструкцию:

```rhai
let value = try {
    parse_float(input.value)
} catch {
    0.0
};
```

Сначала объявляйте переменную, затем присваивайте ей значение внутри отдельного `try`.

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
- нормализуйте строку в локальной переменной перед `parse_int`/`parse_float`;
- перехватывайте ошибки преобразования через отдельный statement-блок `try/catch`;
- возвращайте `status: "error"` с понятным `text` для ошибок формы;
- не смешивайте расчёт и внешние побочные эффекты;
- не помещайте персональные данные в демонстрационные тесты;
- после изменения дождитесь фоновой проверки Match Studio.
