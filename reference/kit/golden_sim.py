#!/usr/bin/env python3
"""Golden-trajectory reference for the Curveball Rust port.
Implements the AS1 ball/enemy/paddle logic exactly as specified in PLAN.md.
Scenario GOLD-1: mouse pinned at world center, level 1, serve at tick 0.
"""
import math

VARA = 31.066017
WL, WT, WR, WB = 25.0, 25.0, 326.0, 226.0
WX, WY = (WR - WL) / 2 + WL, (WB - WT) / 2 + WT  # 175.5, 125.5
DEPTH = 75.0
BALL_D = 30.0
PAD_W, PAD_H = 60.0, 40.0
SPEED, SKILL, CURVE_AMT = 2.0, 17.0, 25.0
CURVE_DECAY = 1.004
WALL_DAMP = (CURVE_DECAY - 1.0) * 50.0 + 1.0  # 1.2


def pip(bx, by, px, py):
    """Exact AS1 quadrant cascade (frame_92 ball enterFrame / mouseDown)."""
    if px + 7 < bx:  return 'UR' if by < py else 'BR'
    if bx < px - 7:  return 'UL' if by < py else 'BL'
    if py + 5 >= by:
        if by >= py - 5: return 'C'
        return 'UR' if bx >= px else 'UL'
    return 'BR' if bx >= px else 'BL'

def s(z): return (90.0 - math.atan(z / VARA) * 180.0 / math.pi) / 90.0
def vis(p, z): return (WX - (WX - p[0]) * s(z), WY - (WY - p[1]) * s(z))
def aabb(c1, w1, h1, c2, w2, h2):
    return (abs(c1[0]-c2[0]) * 2 <= w1+w2) and (abs(c1[1]-c2[1]) * 2 <= h1+h2)

# --- state ---
pad = dict(x=WX, y=WY, sx=0.0, sy=0.0)                       # player paddle (world)
ene = dict(x=WX, y=WY, sx=0.0, sy=0.0)                       # enemy paddle (world)
ball = dict(x=WX, y=WY, z=0.0, vx=0.0, vy=0.0, vz=0.0, cx=0.0, cy=0.0)
pub = dict(bx=WX, by=WY, bz=0.0, dx=0.0, dy=0.0, dz=0.0)     # parent.ballPos/Dir (ball publishes at end of its update)
prev_rect = (vis((WX, WY), 0.0), BALL_D * s(0.0))             # ball's last rendered rect (center, size)
score = dict(total=0, hit=100, curve=50, super=150, acc=100, bdisp=3000, btick=10)
events = []

# --- serve (mouseDown before frame 1) ---
# cached paddle values from previous frame: paddle at center, speed 0
ball['vz'] = SPEED
cx = -0.0 / CURVE_AMT
cy = 0.0 / CURVE_AMT
if abs(cx) < 0.01: cx = 0.01 if pad['x'] < WX else -0.01
if abs(cy) < 0.01: cy = 0.01 if WY < pad['y'] else -0.01
ball['cx'], ball['cy'] = cx, cy
# zone C (|dx|<=7 via thresholds, |dy|<=5): accuracy bonus
score['total'] += score['acc']; score['acc'] = max(0, score['acc'] - 10)
events.append(f"serve: curve=({ball['cx']},{ball['cy']}) score={score['total']} accBonus={score['acc']}")

CHECK = {1,2,3,10,37,38,39,75,76,77,78,114,115,116,117,153,154}
for f in range(1, 161):
    # 1. player paddle (mouse pinned at center)
    tx, ty = WX, WY
    nx = pad['x'] - (pad['x'] - tx) / 1.5
    ny = pad['y'] - (pad['y'] - ty) / 1.5
    ny = min(max(ny, WT + PAD_H/2), WB - PAD_H/2)
    nx = min(max(nx, WL + PAD_W/2), WR - PAD_W/2)
    pad['sx'], pad['sy'] = nx - pad['x'], ny - pad['y']
    pad['x'], pad['y'] = nx, ny

    # 2. enemy paddle (reads previous-frame ball publish)
    if pub['dz'] > 0:
        tx, ty, k = pub['bx'], pub['by'], SKILL
    else:
        tx, ty, k = WX, WY, 15.0
    nx = ene['x'] - (ene['x'] - tx) / k
    ny = ene['y'] - (ene['y'] - ty) / k
    ny = min(max(ny, WT + PAD_H/2), WB - PAD_H/2)
    nx = min(max(nx, WL + PAD_W/2), WR - PAD_W/2)
    ene['sx'], ene['sy'] = nx - ene['x'], ny - ene['y']
    ene['x'], ene['y'] = nx, ny
    ene_rect = (vis((ene['x'], ene['y']), DEPTH), PAD_W * s(DEPTH), PAD_H * s(DEPTH))

    # 3. ball
    ball['vx'] += ball['cx']; ball['vy'] += ball['cy']
    ball['z'] += ball['vz']; ball['x'] += ball['vx']; ball['y'] -= ball['vy']
    if ball['cx'] != 0: ball['cx'] /= CURVE_DECAY
    if ball['cy'] != 0: ball['cy'] /= CURVE_DECAY
    r = BALL_D / 2
    if ball['y'] - r < WT:
        ball['y'] = WT + r; ball['cy'] /= WALL_DAMP; ball['vy'] = -ball['vy']; events.append(f"f{f} wallBounce2(top)")
    elif WB < ball['y'] + r:
        ball['y'] = WB - r; ball['cy'] /= WALL_DAMP; ball['vy'] = -ball['vy']; events.append(f"f{f} wallBounce2(bottom)")
    if ball['x'] - r < WL:
        ball['x'] = WL + r; ball['cx'] /= WALL_DAMP; ball['vx'] = -ball['vx']; events.append(f"f{f} wallBounce1(left)")
    elif WR < ball['x'] + r:
        ball['x'] = WR - r; ball['cx'] /= WALL_DAMP; ball['vx'] = -ball['vx']; events.append(f"f{f} wallBounce1(right)")
    if DEPTH < ball['z']:
        if aabb(prev_rect[0], prev_rect[1], prev_rect[1], ene_rect[0], ene_rect[1], ene_rect[2]):
            ball['z'] = DEPTH
            ball['cx'] = ene['sx'] / CURVE_AMT
            ball['cy'] = -ene['sy'] / CURVE_AMT
            ball['vz'] = -ball['vz']
            events.append(f"f{f} enemy hit pip={pip(ball['x'], ball['y'], ene['x'], ene['y'])}: curve=({ball['cx']:.9g},{ball['cy']:.9g}) eSpeed=({ene['sx']:.9g},{ene['sy']:.9g})")
        else:
            events.append(f"f{f} ENEMY MISS"); break
    elif ball['z'] < 0:
        pad_rect = ((pad['x'], pad['y']), PAD_W, PAD_H)
        if aabb(prev_rect[0], prev_rect[1], prev_rect[1], pad_rect[0], PAD_W, PAD_H):
            dx, dy = ball['x'] - pad['x'], ball['y'] - pad['y']
            zone = pip(ball['x'], ball['y'], pad['x'], pad['y'])
            ball['z'] = 0.0
            ball['cx'] = -pad['sx'] / CURVE_AMT
            ball['cy'] = pad['sy'] / CURVE_AMT
            ball['vz'] = -ball['vz']
            score['total'] += score['hit']; score['hit'] = max(0, score['hit'] - 10)
            if zone == 'C':
                score['total'] += score['acc']; score['acc'] = max(0, score['acc'] - 10)
            if abs(ball['cx']) > 0.1:
                if abs(ball['cy']) > 0.1: score['total'] += score['super']; score['super'] = max(0, score['super'] - 15)
                else: score['total'] += score['curve']; score['curve'] = max(0, score['curve'] - 5)
            elif abs(ball['cy']) > 0.05 or abs(ball['cx']) > 0.05:
                score['total'] += score['curve']; score['curve'] = max(0, score['curve'] - 5)
            events.append(f"f{f} player hit zone={zone} score={score['total']} hit={score['hit']} acc={score['acc']} curve=({ball['cx']:.9g},{ball['cy']:.9g})")
        else:
            events.append(f"f{f} PLAYER MISS"); break
    vp = vis((ball['x'], ball['y']), ball['z'])
    sz = BALL_D * s(ball['z'])
    prev_rect = (vp, sz)
    pub = dict(bx=ball['x'], by=ball['y'], bz=ball['z'], dx=ball['vx'], dy=ball['vy'], dz=ball['vz'])
    if ball['vz'] != 0 and score['bdisp'] > 0:
        score['btick'] -= 1
        if score['btick'] < 0:
            score['btick'] = 10; score['bdisp'] -= 25
    if f in CHECK:
        print(f"f{f:3d}  x={ball['x']:.12g}  y={ball['y']:.12g}  z={ball['z']:.6g}  vx={ball['vx']:.12g}  vy={ball['vy']:.12g}  cx={ball['cx']:.12g}  cy={ball['cy']:.12g}  ex={ene['x']:.12g}  ey={ene['y']:.12g}  bdisp={score['bdisp']}")

print("\nEVENTS:")
for e in events: print(" ", e)
print(f"\nfinal score={score['total']} bonusDisplay={score['bdisp']}")
print(f"s(0)={s(0):.12f} s(75)={s(75):.12f} s(76)={s(76):.12f} s(-2)={s(-2):.12f} wallDamp={WALL_DAMP!r}")
