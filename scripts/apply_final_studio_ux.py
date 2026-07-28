from __future__ import annotations

import base64
import shutil
import zlib
from pathlib import Path

chunks = Path(__file__).with_name(".patch_chunks")
payload = "".join(path.read_text(encoding="utf-8") for path in sorted(chunks.glob("part*.txt")))
try:
    source = zlib.decompress(base64.b64decode(payload)).decode("utf-8")
    source = source.replace(
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub enum MatchKind",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum MatchKind",
        1,
    )
    source = source.replace(
        "'''            match rule.draft.kind {\n                MatchKind::Trigger => {\n''',",
        "'''        for rule in &rules {\n            match rule.draft.kind {\n                MatchKind::Trigger => {\n''',",
        1,
    )
    source = source.replace(
        "'''            let mut variable_names = HashSet::new();",
        "'''        for rule in &rules {\n            let mut variable_names = HashSet::new();",
        1,
    )
    exec(compile(source, str(Path(__file__)), "exec"))
finally:
    shutil.rmtree(chunks, ignore_errors=True)
