"""Parse an ffxivgame.exe minidump: exception + stack walk with symbols.

Scans the crashed thread's captured stack for return addresses into
ffxivgame.exe's .text and annotates each with the nearest preceding
symbol from the research repo's TSV, reconstructing the caller chain
that reached the null-object getter at RVA 0x8EDD44 (#26 crash).
"""

import bisect
import struct
import sys

from minidump.minidumpfile import MinidumpFile

PATH = sys.argv[1]
TSV = r"E:\meteor-reborn-research\docs\re\ghidra_symbols_userdefined.tsv"

syms = []
for line in open(TSV, encoding="utf-8"):
    parts = line.rstrip("\n").split("\t")
    if len(parts) == 2 and len(parts[0]) == 8:
        try:
            syms.append((int(parts[0], 16), parts[1]))
        except ValueError:
            pass
syms.sort()
addrs = [a for a, _ in syms]


def near(va):
    i = bisect.bisect_right(addrs, va) - 1
    if i < 0:
        return "?"
    a, n = syms[i]
    d = va - a
    return "%s+0x%X" % (n, d) if d < 0x4000 else "(sin simbolo cercano)"


mf = MinidumpFile.parse(PATH)
base = next(m.baseaddress for m in mf.modules.modules if "ffxivgame" in m.name.lower())
exc = mf.exception.exception_records[0]
print("crash EIP 0x%08X tid %d" % (exc.ExceptionRecord.ExceptionAddress, exc.ThreadId))

thread = next(t for t in mf.threads.threads if t.ThreadId == exc.ThreadId)
stack_va = thread.Stack.StartOfMemoryRange
reader = mf.get_reader().get_buffered_reader()

# Find the stack segment size from the dump's memory list.
seg_size = None
for seg in mf.memory_segments.memory_segments:
    if seg.start_virtual_address == stack_va:
        seg_size = seg.size
        break
print("stack 0x%08X size 0x%X" % (stack_va, seg_size or 0))

reader.move(stack_va)
data = reader.read(seg_size or 0x10000)

text_lo, text_hi = base + 0x1000, base + 0xB3D000
hits = []
for off in range(0, len(data) - 4, 4):
    (val,) = struct.unpack_from("<I", data, off)
    if text_lo <= val < text_hi:
        hits.append((stack_va + off, val))

print("%d retornos candidatos a .text:" % len(hits))
for sva, val in hits:
    print("  [0x%08X] 0x%08X  %s" % (sva, val, near(val)))
