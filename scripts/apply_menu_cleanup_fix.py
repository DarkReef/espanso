from pathlib import Path

path = Path("espanso-ui/src/win32/native.cpp")
text = path.read_text(encoding="utf-8")
old = '''        TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0,
                       window, NULL);

        break;
'''
new = '''        TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0,
                       window, NULL);
        DestroyMenu(menu);

        break;
'''
if text.count(old) != 1:
    raise RuntimeError(f"context menu cleanup block count: {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
