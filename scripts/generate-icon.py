"""Reproduce the original WinShell vector-like app icon using stdlib only."""
import math
import struct
from pathlib import Path


def segment_distance(x, y, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((x-a[0])*dx + (y-a[1])*dy)/(dx*dx+dy*dy)))
    return math.hypot(x-a[0]-t*dx, y-a[1]-t*dy)


images = []
for n in (16, 24, 32, 48, 64, 128, 256):
    pixels = bytearray()
    for y in reversed(range(n)):
        for x in range(n):
            samples = []
            for sy in (0.25, 0.75):
                for sx in (0.25, 0.75):
                    u, v = (x+sx)/n, (y+sy)/n
                    corner = math.hypot(max(abs(u-.5)-.29, 0), max(abs(v-.5)-.29, 0))
                    if corner > .18:
                        samples.append((0, 0, 0, 0)); continue
                    color = (16, 20, 25, 255)
                    if min(segment_distance(u, v, (.25,.32), (.43,.5)),
                           segment_distance(u, v, (.43,.5), (.25,.68)),
                           segment_distance(u, v, (.51,.68), (.74,.68))) < .04:
                        color = (116, 213, 187, 255)
                    samples.append(color)
            r,g,b,a = [round(sum(c[i] for c in samples)/4) for i in range(4)]
            pixels += bytes((b,g,r,a))
    mask = bytes(((n+31)//32)*4*n)
    header = struct.pack('<IiiHHIIiiII', 40, n, n*2, 1, 32, 0, len(pixels), 0, 0, 0, 0)
    images.append((n, header+pixels+mask))
result = bytearray(struct.pack('<HHH', 0, 1, len(images)))
offset = 6+16*len(images)
for n, data in images:
    result += struct.pack('<BBBBHHII', n % 256, n % 256, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
for _, data in images:
    result += data
Path(__file__).resolve().parent.parent.joinpath('assets/winshell.ico').write_bytes(result)
