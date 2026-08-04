# Предпросмотр форм rEspanso

Форма поддерживает необязательный параметр `preview`. По умолчанию он равен `false`, поэтому старые правила продолжают работать без изменений.

## Предпросмотр самого layout

Этот режим не запускает Rhai. rEspanso просто собирает текст из `layout` и текущих значений полей.

```yaml
matches:
  - trigger: ":preview-demo"
    replace: |
      SCORE: {{patient_form.score}}
      Комментарий: {{patient_form.comment}}
    vars:
      - name: patient_form
        type: form
        params:
          preview: true
          layout: |
            SCORE: [[score]]
            Комментарий: [[comment]]
          fields:
            score:
              type: text
              default: "5"
            comment:
              type: text
              multiline: true
              default: ""
```

Важно: `type: form` возвращает набор значений, а не одну строку. Поэтому после формы используются подполя `{{patient_form.score}}` и `{{patient_form.comment}}`. Запись `{{patient_form}}` некорректна.

При `preview: true` блок «Предпросмотр» обновляется при:

- вводе и удалении текста;
- изменении многострочного поля;
- выборе значения `choice`;
- изменении выбранных элементов `list`.

Поддерживаются поля `text`, `choice` и `list`.

## Короткая запись `form:`

Для встроенной формы флаг располагается рядом с `form`:

```yaml
matches:
  - trigger: ":score-line"
    preview: true
    form: "SCORE: [[score]] %"
    form_fields:
      score:
        type: text
        default: "5"
```

В этом варианте итогом правила становится отрендеренный текст `form`. Для вычислений Rhai используйте полную запись через `vars`.

## Что этот режим не делает

Layout-предпросмотр не выполняет переменные, объявленные после формы. Например, отдельный `type: rhai`, зависящий от формы, будет рассчитан только после закрытия формы.

Для вычисления результата прямо в окне используйте `computed` и реактивный Rhai-предпросмотр: [REACTIVE_RHAI_PREVIEW.ru.md](REACTIVE_RHAI_PREVIEW.ru.md).
