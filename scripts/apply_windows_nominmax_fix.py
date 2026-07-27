from pathlib import Path

path = Path("espanso-ui/src/win32/native.cpp")
text = path.read_text(encoding="utf-8")
if "#define NOMINMAX" not in text:
    marker = "#define UNICODE\n"
    if marker not in text:
        raise RuntimeError("UNICODE marker not found in native.cpp")
    text = text.replace(marker, "#define NOMINMAX\n#define UNICODE\n", 1)
    path.write_text(text, encoding="utf-8")
