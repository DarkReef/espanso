# rEspanso-medlab

Набор медицинских шаблонов и Rhai-калькуляторов для **rEspanso Win v1.0 Unstable**.

## Совместимость

Эта ветка использует возможности unstable-сборки:

- реактивный `computed` внутри формы;
- режимы предпросмотра `live`, `manual` и `submit`;
- чистые placeholders вида `{{result.text}}` без ручного экранирования;
- возврат вычисленных полей после формы как `{{form.result__text}}`;
- удержание формы открытой при `status: "error"`.

Для стабильного Espanso/rEspanso без reactive Rhai preview эти правила не предназначены.

## Структура

```text
match/    # триггеры и формы
scripts/  # локальные Rhai-модули
```

## Контракт реактивного калькулятора

Форма считает результат один раз и использует его и для предпросмотра, и для вставки:

```yaml
- trigger: ":example"
  replace: "{{example_form.result__text}}"
  vars:
    - name: example_form
      type: form
      params:
        preview: true
        preview_mode: live
        preview_debounce_ms: 350
        preview_layout: |
          {{result.text}}
        layout: |
          Значение: [[value]]
        fields:
          value:
            type: text
        computed:
          result:
            type: rhai
            path: "%CONFIG%/scripts/example.rhai"
            function: calculate
            depends_on: []
```

Rhai возвращает map:

```rhai
#{ status: "ok", value: result, text: text }
```

При неверном вводе:

```rhai
#{ status: "error", value: "", text: "Проверьте значение." }
```

Все поля формы приходят строками. Для чисел используется локальная нормализация строки и отдельный statement-блок `try/catch`; выражение `let value = try { ... } catch { ... };` не поддерживается текущим runtime.

## Установка

1. Установите pre-release `rEspanso Win v1.0 Unstable`.
2. Создайте резервную копию пользовательских каталогов `match/` и `scripts/`.
3. Скопируйте каталоги из репозитория в корень конфигурации rEspanso либо импортируйте пакет через Match Studio.
4. Перезапустите rEspanso или выполните штатную перезагрузку конфигурации.

## Важное ограничение

Калькуляторы поддерживают клиническое решение, но не заменяют осмотр, проверку применимости шкалы, действующие клинические рекомендации и профессиональную ответственность врача.

Подробный контракт Rhai и reactive preview находится в документации репозитория `DarkReef/espanso`, каталог `docs/respanso/`.
