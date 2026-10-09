# Реактивный предпросмотр Rhai в формах rEspanso

Форма может вычислять результат Rhai до закрытия и показывать его в блоке
«Предпросмотр». Вычисление использует тот же ограниченный Rhai runtime, что и
обычные переменные `type: rhai`.

```yaml
- name: score_inputs
  type: form
  params:
    preview: true
    preview_mode: live       # live | manual | submit
    preview_debounce_ms: 350
    preview_layout: |
      {{score.text}}
    layout: |
      Возраст: [[age]]
      САД: [[sbp]]
    fields:
      age:
        type: text
        default: "55"
      sbp:
        type: text
        default: "140"
    computed:
      score:
        type: rhai
        path: "%CONFIG%/scripts/score.rhai"
        function: calculate
        depends_on: [age, sbp]
```

Режимы:

- `live` — пересчёт после паузы ввода; длительность задаёт
  `preview_debounce_ms`;
- `manual` — в форме появляется кнопка «Рассчитать»;
- `submit` — Rhai выполняется только перед вставкой.

Каждый вычисляемый узел получает map `input` со всеми текущими полями формы.
Результат следующего узла также доступен по его имени, поэтому `computed`
поддерживает цепочки зависимостей. Циклические зависимости блокируются.

Если Rhai возвращает map, `text` используется как человекочитаемый
предпросмотр, `value` — как основное вычисленное значение. После вставки форма
возвращает вычисленное значение под именем узла (`{{score_inputs.score}}`), а
поля map — с безопасным разделителем, например `score__text`.
