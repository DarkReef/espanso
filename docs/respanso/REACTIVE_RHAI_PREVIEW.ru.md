# Реактивный предпросмотр Rhai в формах rEspanso

Реактивный предпросмотр вычисляет Rhai-скрипт до закрытия формы и показывает результат в блоке «Предпросмотр». После нажатия «Вставить» тот же успешный результат возвращается вместе с полями формы.

## Полный минимальный пример

Файл `scripts/bmi.rhai`:

```rhai
fn fail(message) {
    #{
        status: "error",
        value: "",
        text: message
    }
}

fn calculate(input) {
    let weight = parse_float(input.weight);
    let height_cm = parse_float(input.height);

    if weight <= 0.0 {
        return fail("Укажите массу тела больше 0 кг.");
    }
    if height_cm <= 0.0 {
        return fail("Укажите рост больше 0 см.");
    }

    let height_m = height_cm / 100.0;
    let bmi = round(weight / (height_m * height_m) * 10.0) / 10.0;
    let interpretation = if bmi < 18.5 {
        "Недостаточная масса тела"
    } else if bmi < 25.0 {
        "Нормальная масса тела"
    } else if bmi < 30.0 {
        "Избыточная масса тела"
    } else {
        "Ожирение"
    };

    #{
        status: "ok",
        value: bmi,
        category: interpretation,
        text: `ИМТ: ${bmi} кг/м²\n${interpretation}`
    }
}
```

Файл `match/calculators.yml`:

```yaml
matches:
  - trigger: ":bmi"
    replace: "{{bmi_form.result__text}}"
    vars:
      - name: bmi_form
        type: form
        params:
          preview: true
          preview_mode: live
          preview_debounce_ms: 350
          preview_layout: |
            {{result.text}}
          layout: |
            Масса тела, кг: [[weight]]
            Рост, см: [[height]]
          fields:
            weight:
              type: text
              default: "80"
            height:
              type: text
              default: "175"
          computed:
            result:
              type: rhai
              path: "%CONFIG%/scripts/bmi.rhai"
              function: calculate
              depends_on: []
```

Здесь Rhai выполняется один раз внутри формы. После подтверждения полный текст доступен как `{{bmi_form.result__text}}`, а основное значение — как `{{bmi_form.result}}` или `{{bmi_form.result__value}}`.

## Контракт Rhai-скрипта

Встроенный runtime не исполняет произвольное последнее выражение файла. Он компилирует файл и вызывает функцию:

```rhai
fn calculate(input) {
    // input — map со строковыми значениями полей формы
    "Результат"
}
```

Параметр `function` позволяет выбрать другое имя функции, но по умолчанию используется `calculate`.

Все значения формы приходят строками. Числа нужно разбирать явно:

```rhai
fn parse_number(value) {
    let normalized = value.trim();
    normalized.replace(",", ".");
    parse_float(normalized)
}
```

## Возвращаемое значение

### Строка

```rhai
fn calculate(input) {
    `Возраст: ${input.age}`
}
```

Строка становится и предпросмотром, и основным значением вычисляемого узла.

### Map

Рекомендуемый формат для калькуляторов:

```rhai
fn calculate(input) {
    #{
        status: "ok",
        value: 42,
        text: "Полный человекочитаемый результат",
        category: "Дополнительное поле"
    }
}
```

Для основного значения rEspanso использует первый доступный ключ в порядке:

1. `value`;
2. `text`;
3. `result`.

В `preview_layout` поля map доступны через точку:

```yaml
preview_layout: |
  {{result.text}}
  Категория: {{result.category}}
```

После подтверждения формы они возвращаются с безопасным разделителем `__`:

```text
{{form_name.result}}
{{form_name.result__value}}
{{form_name.result__text}}
{{form_name.result__category}}
```

### Ошибка валидации

Чтобы показать понятную ошибку и не закрывать форму, верните map со статусом `error`:

```rhai
fn calculate(input) {
    if input.age.trim() == "" {
        return #{
            status: "error",
            value: "",
            text: "Укажите возраст."
        };
    }

    #{ status: "ok", value: input.age, text: `Возраст: ${input.age}` }
}
```

При ошибке live/manual-предпросмотр показывает сообщение. При нажатии «Вставить» форма остаётся открытой, пока расчёт не завершится успешно.

## Режимы пересчёта

```yaml
preview_mode: live
preview_debounce_ms: 350
```

- `live` — автоматический пересчёт после паузы ввода;
- `manual` — появляется кнопка «Рассчитать»;
- `submit` — расчёт выполняется только перед вставкой.

`preview_debounce_ms` применяется в режиме `live` и ограничивается диапазоном 50–5000 мс.

`preview: true` нужен только для видимого блока предпросмотра. Если `computed` указан без `preview: true`, вычисления всё равно выполняются при отправке формы и добавляются к её результату.

## Несколько вычисляемых узлов

```yaml
computed:
  bmi:
    type: rhai
    path: "%CONFIG%/scripts/bmi.rhai"
    function: calculate
    depends_on: []
  summary:
    type: rhai
    path: "%CONFIG%/scripts/summary.rhai"
    function: calculate
    depends_on: [bmi]
```

Каждый следующий узел получает:

- все исходные поля формы;
- основное значение предыдущего узла под его именем, например `input.bmi`;
- поля map предыдущего узла с `__`, например `input.bmi__text` и `input.bmi__category`.

`depends_on` определяет порядок только между узлами `computed`. Названия обычных полей формы в этом списке не фильтруют пересчёт: при изменении любого поля в режиме `live` пересчитывается вся цепочка.

Циклическая зависимость между computed-узлами блокирует открытие корректного предпросмотра.

## Пути и ограничения runtime

Разрешены скрипты только внутри каталогов конфигурации и пакетов:

```yaml
path: "%CONFIG%/scripts/calculator.rhai"
```

```yaml
path: "%PACKAGES%/package-name/scripts/calculator.rhai"
```

Абсолютный путь за пределами этих корней отклоняется. В runtime отключены `eval` и `import`; сеть, shell, файловая система и внешние процессы не регистрируются.

Основные лимиты:

- до 100 000 операций;
- глубина вызовов до 32;
- строка до 64 КБ;
- массив до 2048 элементов;
- map до 512 элементов.

## Правильная структура правила

Не рассчитывайте один и тот же скрипт повторно после формы:

```yaml
# Не нужно: второй type: rhai повторяет уже выполненный расчёт.
```

Используйте результат computed-узла напрямую:

```yaml
replace: "{{calculator_form.result__text}}"
```

Так предпросмотр и вставленный текст гарантированно основаны на одном и том же успешном вычислении.
