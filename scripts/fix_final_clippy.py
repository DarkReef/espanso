from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old not in text:
        raise RuntimeError(f"Pattern not found in {path}: {old!r}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


replace_once(
    "espanso-editor/src/diagnostics.rs",
    '''    message
        .split(':')
        .next()
        .map(normalize_message)
        .unwrap_or_else(|| "diagnostic".to_owned())''',
    '''    message
        .split(':')
        .next()
        .map_or_else(|| "diagnostic".to_owned(), normalize_message)''',
)
replace_once(
    "espanso-editor/src/file_monitor.rs",
    "const FULL_AUDIT_INTERVAL: Duration = Duration::from_secs(180);",
    "const FULL_AUDIT_INTERVAL: Duration = Duration::from_mins(3);",
)
replace_once(
    "espanso-editor/src/storm_logo.rs",
    "        let rain_offset = (phase % 4) as f32;",
    "        let rain_offset = [0.0_f32, 1.0, 2.0, 3.0][phase % 4];",
)
