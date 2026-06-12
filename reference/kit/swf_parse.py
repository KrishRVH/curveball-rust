#!/usr/bin/env python3
"""Minimal SWF parser for curveball.swf — extracts the data needed for a faithful port."""
import struct, sys, zlib, json, os

TAGS = {0:"End",1:"ShowFrame",2:"DefineShape",4:"PlaceObject",5:"RemoveObject",6:"DefineBits",
 7:"DefineButton",8:"JPEGTables",9:"SetBackgroundColor",10:"DefineFont",11:"DefineText",
 12:"DoAction",13:"DefineFontInfo",14:"DefineSound",15:"StartSound",17:"DefineButtonSound",
 18:"SoundStreamHead",19:"SoundStreamBlock",20:"DefineBitsLossless",21:"DefineBitsJPEG2",
 22:"DefineShape2",24:"Protect",26:"PlaceObject2",28:"RemoveObject2",32:"DefineShape3",
 33:"DefineText2",34:"DefineButton2",35:"DefineBitsJPEG3",36:"DefineBitsLossless2",
 37:"DefineEditText",39:"DefineSprite",43:"FrameLabel",45:"SoundStreamHead2",46:"DefineMorphShape",
 48:"DefineFont2",56:"ExportAssets",59:"DoInitAction"}

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
    def fb(self, n):  # 16.16 fixed
        return self.sb(n) / 65536.0
    def align(self):
        if self.bit: self.bit = 0; self.byte += 1

def read_rect(b):
    n = b.ub(5)
    r = (b.sb(n), b.sb(n), b.sb(n), b.sb(n))  # xmin xmax ymin ymax (twips)
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
    return dict(scale_x=sx, scale_y=sy, rot0=r0, rot1=r1, tx_twips=tx, ty_twips=ty)

def cstr(data, pos):
    end = data.index(b"\x00", pos)
    return data[pos:end].decode("latin-1"), end + 1

def parse_tags(data, pos, end, out, sprite_id=None):
    frame = 1
    while pos < end:
        code_len = struct.unpack_from("<H", data, pos)[0]; pos += 2
        code, length = code_len >> 6, code_len & 0x3F
        if length == 0x3F:
            length = struct.unpack_from("<I", data, pos)[0]; pos += 4
        body = data[pos:pos + length]; pos += length
        name = TAGS.get(code, f"Tag{code}")
        rec = dict(tag=name, frame=frame, sprite=sprite_id, len=length)
        if code == 1:
            frame += 1; continue
        if code == 0:
            break
        if code == 9:
            rec["rgb"] = "#%02x%02x%02x" % tuple(body[0:3])
        elif code == 43:
            rec["label"] = body.split(b"\x00")[0].decode("latin-1")
        elif code in (2, 22, 32):
            b = Bits(body, 2)
            rec["id"] = struct.unpack_from("<H", body, 0)[0]
            rec["bounds_twips"] = read_rect(b)
            # crude color scan: fillstyle records, RGB(A) triples
            colors = set()
            fs_pos = b.byte
            n_fs = body[fs_pos]; fs_pos += 1
            if n_fs == 0xFF:
                n_fs = struct.unpack_from("<H", body, fs_pos)[0]; fs_pos += 2
            for _ in range(n_fs):
                ftype = body[fs_pos]; fs_pos += 1
                if ftype == 0x00:
                    if code == 32:
                        colors.add("#%02x%02x%02x@%d" % (body[fs_pos], body[fs_pos+1], body[fs_pos+2], body[fs_pos+3])); fs_pos += 4
                    else:
                        colors.add("#%02x%02x%02x" % tuple(body[fs_pos:fs_pos+3])); fs_pos += 3
                else:
                    break  # gradient/bitmap fill — stop crude scan
            rec["fills"] = sorted(colors)
            # line styles follow; scan a couple
            try:
                n_ls = body[fs_pos]; fs_pos += 1
                if n_ls == 0xFF:
                    n_ls = struct.unpack_from("<H", body, fs_pos)[0]; fs_pos += 2
                lines = []
                for _ in range(n_ls):
                    w = struct.unpack_from("<H", body, fs_pos)[0]; fs_pos += 2
                    if code == 32:
                        c = "#%02x%02x%02x@%d" % (body[fs_pos], body[fs_pos+1], body[fs_pos+2], body[fs_pos+3]); fs_pos += 4
                    else:
                        c = "#%02x%02x%02x" % tuple(body[fs_pos:fs_pos+3]); fs_pos += 3
                    lines.append(dict(width_twips=w, color=c))
                rec["lines"] = lines
            except Exception:
                pass
        elif code == 26:
            flags = body[0]
            depth = struct.unpack_from("<H", body, 1)[0]
            p = 3
            rec["depth"] = depth
            if flags & 0x02:
                rec["char_id"] = struct.unpack_from("<H", body, p)[0]; p += 2
            if flags & 0x04:
                rec["matrix"] = read_matrix(Bits(body, p))
                # advance p past matrix by re-reading
                b2 = Bits(body, p); read_matrix(b2); p = b2.byte
            if flags & 0x08:
                # CXFORM with alpha
                b2 = Bits(body, p)
                has_add = b2.ub(1); has_mult = b2.ub(1); n = b2.ub(4)
                vals = [b2.sb(n) for _ in range(((has_mult + has_add) * 4))]
                rec["cxform"] = vals
                b2.align(); p = b2.byte
            if flags & 0x10:
                rec["ratio"] = struct.unpack_from("<H", body, p)[0]; p += 2
            if flags & 0x20:
                rec["name"], p = cstr(body, p)
            rec["has_clipactions"] = bool(flags & 0x80)
        elif code == 14:
            sid = struct.unpack_from("<H", body, 0)[0]
            fl = body[2]
            fmt = (fl >> 4) & 0xF
            rate = (fl >> 2) & 0x3
            bits16 = (fl >> 1) & 1
            stereo = fl & 1
            samples = struct.unpack_from("<I", body, 3)[0]
            rec.update(id=sid, fmt={0:"PCM_BE",1:"ADPCM",2:"MP3",3:"PCM_LE"}.get(fmt, fmt),
                       rate=[5512,11025,22050,44100][rate], bits=16 if bits16 else 8,
                       channels=2 if stereo else 1, samples=samples)
            os.makedirs("/home/claude/swf_out/sounds", exist_ok=True)
            with open(f"/home/claude/swf_out/sounds/sound_{sid}_{rec['fmt']}.bin", "wb") as f:
                f.write(body[7:])
        elif code == 56:
            cnt = struct.unpack_from("<H", body, 0)[0]; p = 2
            exports = []
            for _ in range(cnt):
                cid = struct.unpack_from("<H", body, p)[0]; p += 2
                nm, p = cstr(body, p)
                exports.append((cid, nm))
            rec["exports"] = exports
        elif code == 37:
            sid = struct.unpack_from("<H", body, 0)[0]
            b = Bits(body, 2); bounds = read_rect(b); p = b.byte
            f1, f2 = body[p], body[p+1]; p += 2
            has_text = f1 & 0x80; word_wrap = f1 & 0x40; multiline = f1 & 0x20
            has_font = f1 & 0x01; has_color = f1 & 0x04; has_maxlen = f1 & 0x02
            has_layout = f2 & 0x20; use_outlines = f2 & 0x01
            rec.update(id=sid, bounds_twips=bounds)
            if has_font:
                rec["font_id"] = struct.unpack_from("<H", body, p)[0]; p += 2
                rec["font_height_twips"] = struct.unpack_from("<H", body, p)[0]; p += 2
            if has_color:
                rec["color"] = "#%02x%02x%02x@%d" % tuple(body[p:p+4]); p += 4
            if has_maxlen:
                p += 2
            if has_layout:
                rec["align"] = body[p]; p += 9
            rec["var_name"], p = cstr(body, p)
            if has_text:
                rec["initial_text"], p = cstr(body, p)
        elif code == 48:
            sid = struct.unpack_from("<H", body, 0)[0]
            nlen = body[3]
            rec.update(id=sid, font_name=body[4:4+nlen].decode("latin-1"))
        elif code == 39:
            sid = struct.unpack_from("<H", body, 0)[0]
            fc = struct.unpack_from("<H", body, 2)[0]
            rec.update(id=sid, frames=fc)
            out.append(rec)
            parse_tags(body, 4, len(body), out, sprite_id=sid)
            continue
        elif code in (11, 33):
            rec["id"] = struct.unpack_from("<H", body, 0)[0]
        elif code == 34:
            rec["id"] = struct.unpack_from("<H", body, 0)[0]
        out.append(rec)
    return out

def main(path):
    raw = open(path, "rb").read()
    sig, ver = raw[:3], raw[3]
    flen = struct.unpack_from("<I", raw, 4)[0]
    data = raw[:8] + zlib.decompress(raw[8:]) if sig == b"CWS" else raw
    b = Bits(data, 8)
    stage = read_rect(b)
    pos = b.byte
    frac, integ = data[pos], data[pos+1]
    fps = integ + frac / 256.0
    frames = struct.unpack_from("<H", data, pos + 2)[0]
    hdr = dict(signature=sig.decode(), version=ver, file_len=flen,
               stage_twips=stage, stage_px=[v / 20 for v in stage], fps=fps, frames=frames)
    out = []
    parse_tags(data, pos + 4, len(data), out)
    os.makedirs("/home/claude/swf_out", exist_ok=True)
    with open("/home/claude/swf_out/tags.json", "w") as f:
        json.dump(dict(header=hdr, tags=out), f, indent=1)
    print(json.dumps(hdr, indent=1))
    print(f"{len(out)} tags parsed -> /home/claude/swf_out/tags.json")

if __name__ == "__main__":
    main(sys.argv[1])
