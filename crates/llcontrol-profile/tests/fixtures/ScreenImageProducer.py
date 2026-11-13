#!/usr/bin/env python3
"""Owned two-pixel RGBA PNG for source ingestion tests, using stdlib only."""
import struct,zlib
from pathlib import Path
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
header=chunk(b'IHDR',struct.pack('>IIBBBBB',2,1,8,6,0,0,0))
pixels=bytes([0,200,20,30,128,10,220,30,255])
Path(__file__).with_name('ScreenImage.png').write_bytes(b'\x89PNG\r\n\x1a\n'+header+chunk(b'IDAT',zlib.compress(pixels))+chunk(b'IEND',b''))
