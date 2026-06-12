#!/usr/bin/env python3
"""Deep SWF extraction for curveball.swf — companion to swf_parse.py.

Decodes what the base parser leaves opaque: RemoveObject2 depths, radial
gradient fills (matrix + stops), DefineFont2/DefineFontInfo code tables,
DefineText glyph runs resolved to strings, DefineButton2 records, and
per-frame PlaceObject2 cxforms. This is the source for the lives-pip
corrections, the exact animation tables, the recovered congratulation line,
and the text anchors recorded in DEVIATIONS.md and src/consts.rs.

Usage: python3 swf_deep.py <curveball.swf> [out.json]
"""
import struct, sys, json

class Bits:
    def __init__(self, data, pos=0):
        self.data = data; self.byte = pos; self.bit = 0
    def ub(self, n):
        v = 0
        for _ in range(n):
            v = (v << 1) | ((self.data[self.byte] >> (7 - self.bit)) & 1)
            self.bit += 1
            if self.bit == 8: self.bit = 0; self.byte += 1
        return v
    def sb(self, n):
        v = self.ub(n)
        if n and v & (1 << (n - 1)): v -= 1 << n
        return v
    def fb(self, n): return self.sb(n) / 65536.0
    def align(self):
        if self.bit: self.bit = 0; self.byte += 1

def read_rect(b):
    n = b.ub(5)
    r = (b.sb(n), b.sb(n), b.sb(n), b.sb(n))
    b.align()
    return r

def read_matrix(b):
    sx = sy = 1.0; r0 = r1 = 0.0
    if b.ub(1):
        n = b.ub(5); sx = b.fb(n); sy = b.fb(n)
    if b.ub(1):
        n = b.ub(5); r0 = b.fb(n); r1 = b.fb(n)
    n = b.ub(5); tx = b.sb(n); ty = b.sb(n)
    b.align()
    return dict(sx=sx, sy=sy, r0=r0, r1=r1, tx=tx/20, ty=ty/20)

fonts = {}   # font_id -> dict(codes=[...], advances=[...], ascent, descent)

def parse_font2(body):
    fid = struct.unpack_from("<H", body, 0)[0]
    flags = body[2]
    has_layout = flags & 0x80
    wide_offsets = flags & 0x08
    wide_codes = flags & 0x04
    p = 4  # id(2) + flags(1) + lang(1)
    nlen = body[p]; p += 1
    name = body[p:p+nlen].decode("latin-1"); p += nlen
    nglyphs = struct.unpack_from("<H", body, p)[0]; p += 2
    table_start = p
    osz = 4 if wide_offsets else 2
    fmt = "<I" if wide_offsets else "<H"
    offsets = [struct.unpack_from(fmt, body, p + i*osz)[0] for i in range(nglyphs)]
    p += nglyphs * osz
    code_table_off = struct.unpack_from(fmt, body, p)[0]; p += osz
    cp = table_start + code_table_off
    csz = 2 if wide_codes else 1
    cfmt = "<H" if wide_codes else "<B"
    codes = [struct.unpack_from(cfmt, body, cp + i*csz)[0] for i in range(nglyphs)]
    cp += nglyphs * csz
    out = dict(id=fid, name=name, nglyphs=nglyphs, codes=codes)
    if has_layout:
        asc, desc, lead = struct.unpack_from("<HHh", body, cp); cp += 6
        adv = [struct.unpack_from("<h", body, cp + i*2)[0] for i in range(nglyphs)]
        out.update(ascent=asc/20, descent=desc/20, leading=lead/20, advances=[a/20 for a in adv])
    fonts[fid] = out
    return out

def parse_text(body, code):
    tid = struct.unpack_from("<H", body, 0)[0]
    b = Bits(body, 2)
    bounds = read_rect(b)
    m = read_matrix(b)
    p = b.byte
    glyph_bits, adv_bits = body[p], body[p+1]; p += 2
    runs = []
    cur_font = None; cur_h = None; cur_color = None; x = 0.0; y = 0.0
    while True:
        fb_ = body[p]
        if fb_ == 0:
            p += 1; break
        p += 1
        has_font = fb_ & 0x08; has_color = fb_ & 0x04
        has_y = fb_ & 0x02; has_x = fb_ & 0x01
        if has_font:
            cur_font = struct.unpack_from("<H", body, p)[0]; p += 2
        if has_color:
            if code == 33:
                cur_color = "#%02x%02x%02x@%d" % tuple(body[p:p+4]); p += 4
            else:
                cur_color = "#%02x%02x%02x" % tuple(body[p:p+3]); p += 3
        if has_x:
            x = struct.unpack_from("<h", body, p)[0] / 20; p += 2
        if has_y:
            y = struct.unpack_from("<h", body, p)[0] / 20; p += 2
        if has_font:
            cur_h = struct.unpack_from("<H", body, p)[0] / 20; p += 2
        gc = body[p]; p += 1
        gb = Bits(body, p)
        glyphs = []
        for _ in range(gc):
            gi = gb.ub(glyph_bits)
            adv = gb.sb(adv_bits)
            glyphs.append((gi, adv/20))
        gb.align(); p = gb.byte
        f = fonts.get(cur_font)
        text = "".join(chr(f["codes"][gi]) if f and gi < len(f["codes"]) else "?" for gi, _ in glyphs)
        runs.append(dict(font=cur_font, height=cur_h, color=cur_color, x=x, y=y,
                         text=text, advances=[a for _, a in glyphs]))
        x += sum(a for _, a in glyphs)
    return dict(id=tid, bounds=[v/20 for v in bounds], matrix=m, runs=runs)

def parse_gradient_fill(body, p, shape3):
    ftype = body[p]; p += 1
    if ftype == 0x00:
        if shape3:
            c = "#%02x%02x%02x@%d" % tuple(body[p:p+4]); p += 4
        else:
            c = "#%02x%02x%02x" % tuple(body[p:p+3]); p += 3
        return dict(type="solid", color=c), p
    if ftype in (0x10, 0x12):
        b = Bits(body, p); m = read_matrix(b); p = b.byte
        n = body[p] & 0x0F; p += 1
        recs = []
        for _ in range(n):
            ratio = body[p]; p += 1
            if shape3:
                c = "#%02x%02x%02x@%d" % tuple(body[p:p+4]); p += 4
            else:
                c = "#%02x%02x%02x" % tuple(body[p:p+3]); p += 3
            recs.append(dict(ratio=ratio, color=c))
        return dict(type="radial" if ftype == 0x12 else "linear", matrix=m, stops=recs), p
    return dict(type=f"other_{ftype:#x}"), p

def parse_shape(body, code):
    sid = struct.unpack_from("<H", body, 0)[0]
    b = Bits(body, 2)
    bounds = read_rect(b)
    p = b.byte
    shape3 = code == 32
    nf = body[p]; p += 1
    if nf == 0xFF:
        nf = struct.unpack_from("<H", body, p)[0]; p += 2
    fills = []
    for _ in range(nf):
        f, p = parse_gradient_fill(body, p, shape3)
        fills.append(f)
    nl = body[p]; p += 1
    if nl == 0xFF:
        nl = struct.unpack_from("<H", body, p)[0]; p += 2
    lines = []
    for _ in range(nl):
        w = struct.unpack_from("<H", body, p)[0]; p += 2
        if shape3:
            c = "#%02x%02x%02x@%d" % tuple(body[p:p+4]); p += 4
        else:
            c = "#%02x%02x%02x" % tuple(body[p:p+3]); p += 3
        lines.append(dict(width=w/20, color=c))
    return dict(id=sid, bounds=[v/20 for v in bounds], fills=fills, lines=lines)

def parse_button2(body):
    bid = struct.unpack_from("<H", body, 0)[0]
    # flags u8, ActionOffset u16
    p = 5
    recs = []
    while body[p] != 0:
        flags = body[p]; p += 1
        cid = struct.unpack_from("<H", body, p)[0]; p += 2
        depth = struct.unpack_from("<H", body, p)[0]; p += 2
        b = Bits(body, p); m = read_matrix(b); p = b.byte
        # cxform with alpha
        b = Bits(body, p)
        has_add = b.ub(1); has_mult = b.ub(1); n = b.ub(4)
        for _ in range((has_add + has_mult) * 4): b.sb(n)
        b.align(); p = b.byte
        recs.append(dict(states=flags, char=cid, depth=depth, matrix=m))
    return dict(id=bid, records=recs)

def parse_cxform(b, alpha):
    has_add = b.ub(1); has_mult = b.ub(1); n = b.ub(4)
    k = 4 if alpha else 3
    mult = [b.sb(n) for _ in range(k)] if has_mult else None
    add = [b.sb(n) for _ in range(k)] if has_add else None
    b.align()
    return dict(mult=mult, add=add)

def parse_tags(data, pos, end, out, sprite_id=None):
    frame = 1
    while pos < end:
        code_len = struct.unpack_from("<H", data, pos)[0]; pos += 2
        code, length = code_len >> 6, code_len & 0x3F
        if length == 0x3F:
            length = struct.unpack_from("<I", data, pos)[0]; pos += 4
        body = data[pos:pos + length]; pos += length
        if code == 1:
            frame += 1; continue
        if code == 0:
            break
        rec = dict(frame=frame, sprite=sprite_id)
        if code == 28:
            rec.update(tag="RemoveObject2", depth=struct.unpack_from("<H", body, 0)[0])
        elif code == 48:
            rec.update(tag="DefineFont2", **parse_font2(body))
        elif code in (11, 33):
            rec.update(tag="DefineText", **parse_text(body, code))
        elif code in (2, 22, 32):
            rec.update(tag="DefineShape", **parse_shape(body, code))
        elif code == 34:
            rec.update(tag="DefineButton2", **parse_button2(body))
        elif code == 26:
            flags = body[0]
            depth = struct.unpack_from("<H", body, 1)[0]
            p = 3
            rec.update(tag="PlaceObject2", depth=depth, move=bool(flags & 0x01))
            if flags & 0x02:
                rec["char"] = struct.unpack_from("<H", body, p)[0]; p += 2
            if flags & 0x04:
                b2 = Bits(body, p); rec["matrix"] = read_matrix(b2); p = b2.byte
            if flags & 0x08:
                b2 = Bits(body, p); rec["cxform"] = parse_cxform(b2, True); p = b2.byte
        elif code == 39:
            sid = struct.unpack_from("<H", body, 0)[0]
            parse_tags(body, 4, len(body), out, sprite_id=sid)
            continue
        else:
            continue
        out.append(rec)
    return out

def main(path):
    raw = open(path, "rb").read()
    data = raw
    b = Bits(data, 8)
    read_rect(b)
    pos = b.byte + 4
    out = []
    parse_tags(data, pos, len(data), out)
    out_path = sys.argv[2] if len(sys.argv) > 2 else "swf_deep.json"
    json.dump(out, open(out_path, "w"), indent=1)
    print(f"{len(out)} records -> {out_path}")

if __name__ == "__main__":
    main(sys.argv[1])
