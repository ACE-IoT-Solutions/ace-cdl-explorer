#!/usr/bin/env python3
"""Reject a WASM bundle whose wasm-bindgen externref export points at a funcref table.

An older Binaryen build reordered the tables in our Pages artifact and left
``__wbindgen_externrefs`` pointing at table 0. The JavaScript glue then failed
at startup when it tried to store ``undefined`` in that function table.
"""

import pathlib
import sys


def unsigned_leb(data: bytes, offset: int) -> tuple[int, int]:
    value = shift = 0
    while True:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, offset
        shift += 7


def section(data: bytes, wanted: int) -> bytes:
    offset = 8
    while offset < len(data):
        kind = data[offset]
        size, start = unsigned_leb(data, offset + 1)
        if kind == wanted:
            return data[start : start + size]
        offset = start + size
    raise ValueError(f"WASM section {wanted} is missing")


def table_types(data: bytes) -> list[int]:
    entries = section(data, 4)
    count, offset = unsigned_leb(entries, 0)
    types = []
    for _ in range(count):
        types.append(entries[offset])
        offset += 1
        flags, offset = unsigned_leb(entries, offset)
        _, offset = unsigned_leb(entries, offset)
        if flags & 1:
            _, offset = unsigned_leb(entries, offset)
    return types


def externref_export_index(data: bytes) -> int:
    entries = section(data, 7)
    count, offset = unsigned_leb(entries, 0)
    for _ in range(count):
        length, offset = unsigned_leb(entries, offset)
        name = entries[offset : offset + length].decode()
        offset += length
        kind = entries[offset]
        offset += 1
        index, offset = unsigned_leb(entries, offset)
        if name == "__wbindgen_externrefs":
            if kind != 1:
                raise ValueError("__wbindgen_externrefs is not exported as a table")
            return index
    raise ValueError("__wbindgen_externrefs export is missing")


def main(path: str) -> None:
    data = pathlib.Path(path).read_bytes()
    if data[:8] != b"\x00asm\x01\x00\x00\x00":
        raise ValueError("Invalid WebAssembly header")
    types = table_types(data)
    index = externref_export_index(data)
    if index >= len(types) or types[index] != 0x6F:
        raise ValueError(
            f"__wbindgen_externrefs points at table {index}, "
            f"whose reference type is {types[index] if index < len(types) else 'missing'}; "
            "expected externref (0x6f)"
        )
    print(f"Verified wasm-bindgen externref export at table {index}")


if __name__ == "__main__":
    main(sys.argv[1])
