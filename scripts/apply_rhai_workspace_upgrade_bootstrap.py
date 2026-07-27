from __future__ import annotations

import base64
import gzip
from pathlib import Path


def main() -> None:
    script_path = Path(__file__).resolve()
    chunk_dir = script_path.parent / ".rhai_upgrade_chunks"
    chunks = sorted(chunk_dir.glob("*.txt"))
    if not chunks:
        raise RuntimeError(f"No upgrade chunks found in {chunk_dir}")
    payload = "".join(path.read_text(encoding="utf-8").strip() for path in chunks)
    source = gzip.decompress(base64.b64decode(payload)).decode("utf-8")
    namespace = {"__name__": "__main__", "__file__": str(script_path)}
    exec(compile(source, "<rhai-workspace-upgrade>", "exec"), namespace)


if __name__ == "__main__":
    main()
