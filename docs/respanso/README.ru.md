# Документация rEspanso

rEspanso — локальный форк Espanso с portable-запуском Windows, Match Studio, встроенной средой Rhai и грозовым интерфейсом.

## С чего начать

1. [Быстрый старт](QUICK_START.ru.md)
2. [Match Studio](MATCH_STUDIO.ru.md)
3. [Перенос правил и скриптов](CONFIG_TRANSFER.ru.md)
4. [Rhai для rEspanso](RHAI_GUIDE.ru.md)
5. [Предпросмотр обычных форм](FORM_PREVIEW.ru.md)
6. [Реактивный Rhai-предпросмотр](REACTIVE_RHAI_PREVIEW.ru.md)
7. [Промпты для генерации и аудита Rhai](RHAI_PROMPT.ru.md)
8. [Решение проблем](TROUBLESHOOTING.ru.md)

## Что хранится рядом с portable-приложением

- `config/` — внутренние настройки конкретной установки;
- `match/` — YAML-правила и `base.yml`;
- `scripts/` — Rhai-скрипты;
- `packages/` и `runtime/` — служебные данные движка;
- `config-packages/` — экспортированные пакеты правил и скриптов.

Внутренний `config/` не переносится пакетами Studio: настройки одной машины не должны перезаписывать настройки другой.
