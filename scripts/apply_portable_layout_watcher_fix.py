import base64
import gzip
import hashlib
from pathlib import Path

chunk_dir = Path(__file__).with_name(".portable_patch_chunks")
encoded = "".join(path.read_text(encoding="ascii").strip() for path in sorted(chunk_dir.glob("*.txt")))
expected = "4a57290d78081a9c09e6e7c157a9a633064033c07c3c1adb579824afc126a972"
actual = hashlib.sha256(encoded.encode("ascii")).hexdigest()
if actual != expected:
    raise RuntimeError(f"portable patch payload checksum mismatch: {actual}")
source = gzip.decompress(base64.b64decode(encoded))
exec(compile(source, __file__, "exec"))
