#!/usr/bin/env python3
"""Generate tiny, deterministic malicious-metadata documents for the ZIP regression tests."""
from io import BytesIO
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def hostile(entries):
    output = BytesIO()
    with zipfile.ZipFile(output, 'w') as archive:
        for name, content in entries:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o106777 << 16  # SUID/SGID on ordinary document parts.
            archive.writestr(info, content)
        for name, mode, content in [
            ('../outside/sentinel', 0o100644, b'overwritten'),
            ('escape', 0o120777, b'../outside'),
            ('escape/sentinel', 0o100644, b'overwritten'),
        ]:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = mode << 16
            archive.writestr(info, content)
    return output.getvalue()


def main():
    docx = ROOT / 'crates/okbase-convert/tests/data/hostile.docx'
    docx.parent.mkdir(parents=True, exist_ok=True)
    docx.write_bytes(hostile([('word/document.xml', b'''<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Safe document content</w:t></w:r></w:p></w:body></w:document>''')]))
    xlsx = ROOT / 'crates/okbase-data/tests/data/hostile.xlsx'
    with zipfile.ZipFile(xlsx.with_name('stock.xlsx')) as original:
        xlsx.write_bytes(hostile([(name, original.read(name)) for name in original.namelist()]))


if __name__ == '__main__':
    main()
