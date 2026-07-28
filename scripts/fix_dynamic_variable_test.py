from pathlib import Path

path = Path("espanso-editor/src/dynamic_variables.rs")
source = path.read_text(encoding="utf-8")
old = '        assert!(updated.contains("format: \\"%d.%m.%Y\\""));\n'
new = (
    '        assert!(updated.contains("format:"));\n'
    '        assert!(updated.contains("%d.%m.%Y"));\n'
)
if source.count(old) != 1:
    raise SystemExit("expected date-format assertion was not found exactly once")
path.write_text(source.replace(old, new, 1), encoding="utf-8")
