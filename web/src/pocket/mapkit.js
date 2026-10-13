// What a map's layout adds to the reference's plan, built with the reference's kit.
//
// The reference's layout describes a street: buildings, open ground, a gate and set pieces, all on one
// level. A map of ours may also state (maps/README.md):
//
//   GROUND   open ground with none of the junk the reference leaves in an alley
//   WALLS    free walls and the lintels over their doorways
//   COVERED  ways under a roof with a mass over it: a passage through a block
//   RAISED   ground a step or a storey above the rest, and RAMPS up to it
//   COVER    crates, stacks, blocks and drums to fight from, each with its collision
//
// Every piece is the kit's: its boxes (`BOX`, `BOX_SOFT`), its materials by their palette keys, its
// parapet, its bulbs, its prop prototypes, and the Assembler's collision proxies. Nothing here draws a
// surface of its own.
import * as THREE from 'three';
import { BOX, BOX_SOFT, IDENT, LL, parapet } from '../world/kit.js';
import { hangingBulb, stackCrates } from '../world/interiors.js';
import { fillMasks } from '../world/util.js';

const _m = new THREE.Matrix4();

/** A wedge `len` long (+X), `h` high at its far end, `w` wide (Z), its foot on y = 0 and centred on x and z. */
function wedge(len, h, w) {
  const x0 = -len / 2, x1 = len / 2, z0 = -w / 2, z1 = w / 2;
  // (the slope, the two flanks, the tall end; no underside: it stands on the ground)
  const quads = [
    [[x0, 0, z1], [x1, h, z1], [x1, h, z0], [x0, 0, z0]],
    [[x1, 0, z1], [x1, 0, z0], [x1, h, z0], [x1, h, z1]],
  ];
  const tris = [
    [[x0, 0, z1], [x1, 0, z1], [x1, h, z1]],
    [[x0, 0, z0], [x1, h, z0], [x1, 0, z0]],
  ];
  const pos = [];
  for (const [a, b, c, d] of quads) pos.push(...a, ...b, ...c, ...a, ...c, ...d);
  for (const [a, b, c] of tris) pos.push(...a, ...b, ...c);
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  geo.computeVertexNormals();
  fillMasks(geo, 0.35, 0.55, 0.2);
  return geo;
}

/** The turn (about Y) that carries a wedge's rise from +X to a compass side. */
const RISE = { 'x+': 0, 'z-': Math.PI / 2, 'x-': Math.PI, 'z+': -Math.PI / 2 };

/**
 * Builds the layout's additions. Returns `{ groundAt(x, z), ceilings, barriers }`: the height of the ground
 * the additions made at a level point (0 where there is none), which is what stands a piece of cover on a
 * platform; the undersides the additions put over the ground (`{ rect, y }` in level space) and the walls
 * that stand on it (`{ a, b, t }`), which is what the walk grid needs (./mapnav.js).
 */
export function buildExtras(A, rng, layout) {
  const raised = layout.RAISED ?? [];
  const ramps = layout.RAMPS ?? [];
  const inside = ([x0, z0, x1, z1], x, z) => x >= x0 && x <= x1 && z >= z0 && z <= z1;
  const groundAt = (x, z) => {
    let y = 0;
    for (const p of raised) if (inside(p.rect, x, z)) y = Math.max(y, p.h);
    for (const r of ramps) {
      if (!inside(r.rect, x, z)) continue;
      const [x0, z0, x1, z1] = r.rect;
      const t = r.up === 'x+' ? (x - x0) / (x1 - x0) : r.up === 'x-' ? (x1 - x) / (x1 - x0) : r.up === 'z+' ? (z - z0) / (z1 - z0) : (z1 - z) / (z1 - z0);
      y = Math.max(y, (r.h0 ?? 0) + (r.h - (r.h0 ?? 0)) * t);
    }
    return y;
  };

  const ceilings = [], barriers = [];

  // ------------------------------------------------------------------ open ground
  // The slab the reference lays under an alley, and none of what it leaves in one.
  for (const g of layout.GROUND ?? []) {
    const [x0, z0, x1, z1] = g.rect;
    A.add(g.surface ?? 'dirt', BOX(A), LL(IDENT, (x0 + x1) / 2, 0.03, (z0 + z1) / 2, 0, x1 - x0, 0.06, z1 - z0), { masks: [0.2, 0.6, 0.35] });
    A.box(A.surfaceOf(g.surface ?? 'dirt'), (x0 + x1) / 2, 0.02, (z0 + z1) / 2, x1 - x0, 0.05, z1 - z0);
  }

  // ------------------------------------------------------------------ raised ground
  for (const p of raised) {
    const [x0, z0, x1, z1] = p.rect;
    const cx = (x0 + x1) / 2, cz = (z0 + z1) / 2, w = x1 - x0, d = z1 - z0;
    // The body goes a little into the ground, so the sand's swell never shows under it.
    const foot = 0.6;
    A.add(p.key ?? 'concrete', BOX(A), LL(IDENT, cx, (p.h - foot) / 2, cz, 0, w, p.h + foot, d), { masks: [0.5, 0.7, 0.4] });
    // A course of another surface on top (a yard's sand on a concrete terrace).
    if (p.top) A.add(p.top, BOX(A), LL(IDENT, cx, p.h + 0.02, cz, 0, w - 0.3, 0.06, d - 0.3), { masks: [0.2, 0.6, 0.35] });
    A.box(A.surfaceOf(p.top ?? p.key ?? 'concrete'), cx, (p.h - foot) / 2 + (p.top ? 0.025 : 0), cz, w, p.h + foot + (p.top ? 0.05 : 0), d);
  }
  for (const r of ramps) {
    const [x0, z0, x1, z1] = r.rect;
    const along = r.up[0] === 'x';
    const len = along ? x1 - x0 : z1 - z0, w = along ? z1 - z0 : x1 - x0;
    const h0 = r.h0 ?? 0, rise = r.h - h0;
    const geo = wedge(len, rise, w);
    _m.compose(new THREE.Vector3((x0 + x1) / 2, h0 - 0.02, (z0 + z1) / 2), new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), RISE[r.up]), new THREE.Vector3(1, 1, 1));
    A.add(r.key ?? 'concrete', geo, _m, { masks: [0.45, 0.6, 0.3] });
    A.collideGeo(A.surfaceOf(r.key ?? 'concrete'), geo, _m);
    geo.dispose();
    // A ramp that starts above the ground stands on a body of its own.
    if (h0 > 0) {
      A.add(r.key ?? 'concrete', BOX(A), LL(IDENT, (x0 + x1) / 2, (h0 - 0.6) / 2, (z0 + z1) / 2, 0, x1 - x0, h0 + 0.6, z1 - z0), { masks: [0.5, 0.7, 0.4] });
      A.box('concrete', (x0 + x1) / 2, (h0 - 0.6) / 2, (z0 + z1) / 2, x1 - x0, h0 + 0.6, z1 - z0);
    }
  }

  // ------------------------------------------------------------------ walls and lintels
  for (const wall of layout.WALLS ?? []) {
    const [ax, az] = wall.a, [bx, bz] = wall.b;
    const dx = bx - ax, dz = bz - az;
    const len = Math.hypot(dx, dz);
    const ry = Math.atan2(dx, dz) - Math.PI / 2;
    const cx = (ax + bx) / 2, cz = (az + bz) / 2;
    const y0 = wall.y ?? groundAt(cx, cz), h = wall.h ?? 3.4, t = wall.t ?? 0.4;
    A.add(wall.key ?? 'plaster_sand', BOX(A), LL(IDENT, cx, y0 + h / 2, cz, ry, len, h, t), { masks: [0.5, 0.7, 0.4] });
    if (wall.coping !== false) A.add('concrete', BOX_SOFT(A), LL(IDENT, cx, y0 + h + 0.06, cz, ry, len + 0.14, 0.12, t + 0.14), { masks: [0.8, 0.4, 0.15] });
    A.box('concrete', cx, y0 + h / 2, cz, len, h, t + 0.05, ry);
    // A wall that starts above the ground is a lintel: the ground under it is a doorway. Any other wall
    // is a barrier to the walk grid.
    if (wall.y === undefined || wall.y - groundAt(cx, cz) < 0.5) barriers.push({ a: wall.a, b: wall.b, t });
    else if (wall.y - groundAt(cx, cz) >= 1.9) {
      if (Math.abs(dx) > 1e-6 && Math.abs(dz) > 1e-6) throw new Error(`WALLS: the lintel from ${wall.a} to ${wall.b} runs along neither x nor z`);
      const half = t / 2 + 0.025;
      ceilings.push({ rect: [Math.min(ax, bx) - (dx ? 0 : half), Math.min(az, bz) - (dz ? 0 : half), Math.max(ax, bx) + (dx ? 0 : half), Math.max(az, bz) + (dz ? 0 : half)], y: wall.y });
    }
  }

  // ------------------------------------------------------------------ covered ways
  for (const way of layout.COVERED ?? []) {
    const [x0, z0, x1, z1] = way.rect;
    const cx = (x0 + x1) / 2, cz = (z0 + z1) / 2, w = x1 - x0, d = z1 - z0;
    const y = way.y ?? 3.3, mass = way.mass ?? 3.0, slab = 0.32;
    // The soffit a walker looks up at, then the block that stands on it.
    A.add(way.soffit ?? 'concrete_dark', BOX(A), LL(IDENT, cx, y + slab / 2, cz, 0, w, slab, d), { masks: [0.3, 0.85, 0.7] });
    if (mass > 0) {
      A.add(way.key ?? 'plaster_sand', BOX(A), LL(IDENT, cx, y + slab + mass / 2, cz, 0, w + 0.1, mass, d + 0.1), { masks: [0.5, 0.6, 0.3] });
      A.add('concrete', BOX_SOFT(A), LL(IDENT, cx, y + slab + 0.08, cz, 0, w + 0.3, 0.16, d + 0.3), { masks: [0.8, 0.5, 0.25] });
      A.add('roof_screed', BOX(A), LL(IDENT, cx, y + slab + mass + 0.02, cz, 0, w - 0.2, 0.06, d - 0.2), { masks: [0.45, 0.3, 0.15] });
      parapet(A, way.key ?? 'plaster_sand', cx, cz, w + 0.1, d + 0.1, y + slab + mass, rng, { h: 0.62, t: 0.22 });
    }
    A.box('concrete', cx, y + (slab + mass) / 2, cz, w, slab + mass, d);
    ceilings.push({ rect: way.rect, y });
    for (const [bx, bz] of way.bulbs ?? []) hangingBulb(A, rng, bx, y, bz);
  }

  // ------------------------------------------------------------------ cover
  // Loose things are knocked a little out of true, as the reference's dressing knocks its own.
  const was = A.jitter;
  A.jitter = { rng: rng.fork(), yaw: 0.06, scale: 0.03 };
  for (const [kind, x, z, a = 0, b = 1] of layout.COVER ?? []) {
    const y = groundAt(x, z) + 0.03;
    if (kind === 'crate') {
      // One big crate: the kit's largest, grown to `b` metres along its side.
      const s = b / 0.82;
      A.put('crate_c', x, y, z, a, s, [1, 1.1, 1]);
      A.box('wood', x, y + (0.82 * 0.85 * s) / 2, z, 0.82 * s, 0.82 * 0.85 * s, 0.82 * 0.92 * s, a);
    } else if (kind === 'stack') {
      stackCrates(A, rng, x, y, z, a || 3);
    } else if (kind === 'block') {
      A.put('block_big', x, y, z, a, b, [1, 1.2, 1]);
      A.box('concrete', x, y + 0.475 * b, z, 1.25 * b, 0.95 * b, 0.85 * b, a);
    } else if (kind === 'drums') {
      const n = a || 3;
      for (let i = 0; i < n; i++) {
        const t = (i / n) * 6.283 + rng.range(-0.3, 0.3);
        const px = x + Math.cos(t) * 0.42 * (n > 1 ? 1 : 0), pz = z + Math.sin(t) * 0.42 * (n > 1 ? 1 : 0);
        A.put(rng.pick(['barrel_rust', 'barrel_blue', 'barrel_rust']), px, y, pz, rng.float() * 6.28, 1, [1, 1.2, 1]);
      }
      A.box('metal', x, y + 0.45, z, n > 1 ? 1.5 : 0.62, 0.9, n > 1 ? 1.5 : 0.62);
    } else {
      throw new Error(`COVER: no kind "${kind}"`);
    }
  }
  A.jitter = was;

  return { groundAt, ceilings, barriers };
}
