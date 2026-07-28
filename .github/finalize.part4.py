  &definition,
    )
    .unwrap();
    assert!(added);
    assert!(updated.contains("type: \"echo\""));
    assert!(updated.contains("Куцин Иван Юрьевич"));
}

#[test]
fn detects_manually_typed_builtin_placeholders() {
    let definitions = builtin_definitions_in("{{date}} {{time}} {{clipboard}}");
    let names = definitions
        .iter()
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["date", "time", "clipboard"]);
}
''', 4)
test_module_end = dynamic.rfind("\n}")
if test_module_end < 0:
    raise SystemExit("dynamic variable test module end not found")
dynamic = dynamic[:test_module_end] + "\n" + alias_tests + dynamic[test_module_end:]
dynamic_path.write_text(dynamic, encoding="utf-8")


docs_path = Path("docs/respanso/MATCH_STUDIO.ru.md")
docs = docs_path.read_text(encoding="utf-8")
marker = "## Динамические переменные в редакторе правил"
if marker in docs:
    docs = docs.split(marker, 1)[0].rstrip() + "\n\n"
docs += textwrap.dedent('''
## YAML-файлы

В блоке «YAML-файлы» доступны кнопки «Создать файл» и «Удалить файл». Новый файл создаётся в папке `match` с безопасным именем и начальным правилом `:new`. Основной `base.yml`/`base.yaml` удалить нельзя. Перед удалением Studio требует отсутствие несохранённых изменений.

## Редактор правил

Структурная вкладка называется «Редактор». Отдельная кнопка «Применить изменения» больше не нужна: правки сразу попадают в рабочую копию, но остаются несохранёнными до `Ctrl+S`. Функция «Выключить триггер / правило» удалена.

## Динамические переменные в редакторе правил

Кнопка `?` рядом с текстом подстановки создаёт готовые переменные или собственную переменную. Поддерживаемые встроенные типы движка: `date`, `clipboard`, `echo`, `choice`, `form`, `random`, `rhai`, `script`, `shell`. Для удобства Studio также понимает псевдотипы:

- `time` — преобразуется в `date` с форматом `%H:%M`;
- `string` — преобразуется в `echo`, поэтому можно создать `{{doc}}` со значением `Куцин Иван Юрьевич`.

Ручной ввод `{{date}}`, `{{time}}`, `{{weekday}}` или `{{clipboard}}` в тексте автоматически создаёт недостающий блок `vars`. Запись на диск выполняется только после `Ctrl+S`.

В справке также указан глобальный хоткей `Ctrl+Alt+M` — поиск триггера по выделенному тексту.
''').lstrip()
docs_path.write_text(docs, encoding="utf-8")


final_workflow = r'''name: Windows x64 portable

on:
  pull_request:
    branches:
      - rEspanso-feature
    paths-ignore:
      - "docs/**"
      - "**/*.md"
      - "**/*.txt"
      - "LICENSE*"
  workflow_dispatch:
  release:
    types:
      - published

permissions:
  contents: read

concurrency:
  group: __WINDOWS_CONCURRENCY_GROUP__
  cancel-in-progress: true

jobs:
  verify-and-build:
    name: Check and build rEspanso portable
    runs-on: windows-2022
    timeout-minutes: 90
    env:
      CARGO_TERM_COLOR: always
      RUST_BACKTRACE: 1

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up MSVC x64
        uses: ilammy/msvc-dev-cmd@v1
        with:
          arch: x64

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc
          components: rustfmt, clippy

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: windows-x64-portable
          cache-on-failure: true

      - name: Verify repository cleanup
        shell: pwsh
        run: |
          $workflows = @(Get-ChildItem ".github/workflows" -File)
          if ($workflows.Count -ne 1 -or $workflows[0].Name -ne "windows-x64-portable.yml") {
            throw "Repository must contain exactly one final workflow"
          }
          if (Test-Path ".claude") {
            throw ".claude must not be present"
          }
          $junk = @(git ls-files | Where-Object {
            ($_ -match '^(scripts|\.github)/.*(^|[-_.])(apply|patch|trigger|diagnose|rerun|temporary|temp|fix_dynamic|final[-_]?build)([-_.]|$)') -or
            ($_ -match '(^|/)trigger[^/]*$')
          })
          if ($junk.Count -gt 0) {
            $junk | ForEach-Object { Write-Error "Technical junk remains: $_" }
            throw "Technical cleanup is incomplete"
          }

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run tests
        run: |
          cargo test --locked --quiet -p espanso-editor
          cargo test --locked --quiet -p espanso-engine
          cargo test --locked --quiet -p espanso-render
          cargo test --locked --quiet -p espanso --bin respanso-portable

      - name: Run Clippy
        run: cargo clippy --locked -p espanso -p espanso-editor --all-targets -- --deny warnings

      - name: Build release binaries
        run: |
          cargo build --locked --quiet --release -p espanso --bin espanso --bin respanso-portable
          cargo build --locked --quiet --release -p espanso-editor

      - name: Assemble portable archive
        shell: pwsh
        run: |
          $env:EXEC_PATH = (Resolve-Path "target/release/espanso.exe").Path
          $env:LAUNCHER_PATH = (Resolve-Path "target/release/respanso-portable.exe").Path
          $env:EDITOR_PATH = (Resolve-Path "target/release/espanso-editor.exe").Path
          & "./scripts/build_windows_native_portable_with_studio.ps1"
          $archive = "target/windows/rEspanso-Native-Portable-with-Studio-Windows-x86_64.zip"
          if (-not (Test-Path $archive)) {
            throw "Portable archive was not created"
          }
          $entries = tar -tf $archive
          if ($entries -match '\.(bat|cmd)$') {
            throw "Portable archive contains BAT/CMD launchers"
          }
          $required = @("rEspanso.exe", "rEspanso-core.exe", "rEspanso Match Studio.exe")
          foreach ($file in $required) {
            if (-not ($entries -match [regex]::Escape($file))) {
              throw "Portable archive is missing $file"
            }
          }

      - name: Upload portable archive
        uses: actions/upload-artifact@v4
        with:
          name: rEspanso-Windows-x64-portable
          path: target/windows/rEspanso-Native-Portable-with-Studio-Windows-x86_64.zip
          if-no-files-found: error
          retention-days: 14
          compression-level: 0
'''
final_workflow = final_workflow.replace(
    "__WINDOWS_CONCURRENCY_GROUP__",
    "windows-x64-portable-$" + "{{ github.event.pull_request.number || github.ref }}",
)
workflows_dir = Path(".github/workflows")
workflows_dir.mkdir(parents=True, exist_ok=True)
final_workflow_path = workflows_dir / "windows-x64-portable.yml"
final_workflow_path.write_text(final_workflow, encoding="utf-8")
for workflow in workflows_dir.iterdir():
    if workflow.is_file() and workflow != final_workflow_path:
        workflow.unlink()

Path(".github/finalize.payload").unlink(missing_ok=True)
Path(".github/diagnose-finalize.log").unlink(missing_ok=True)
Path("finalize.log").unlink(missing_ok=True)
shutil.rmtree(".claude", ignore_errors=True)
tracked = subprocess.check_output(["git", "ls-files"], text=True).splitlines()
junk_name = re.compile(
    r"(^|[-_.])(apply|patch|trigger|diagnose|rerun|temporary|temp|fix_dynamic|final[-_]?build)([-_.]|$)",
    re.IGNORECASE,
)
for relative in tracked:
    path = Path(relative)
    lowered = relative.lower().replace("\\", "/")
    technical_scope = lowered.startswith("scripts/") or lowered.startswith(".github/")
    trigger_file = path.name.lower().startswith("trigger")
    if (technical_scope and junk_name.search(path.name)) or trigger_file:
        if path.exists() and path != final_workflow_path:
            path.unlink()

subprocess.run(["git", "diff", "--check"], check=True)
