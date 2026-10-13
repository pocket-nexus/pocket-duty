// The kit that states a place freely (maps/<id>/place.js): floors at any height, the walls that close
// them, covered ways, doors, crates, houses. No street, no gate, no perimeter of a fixed size.
//
// A place is read in its own coordinates: metres east and north of its middle, heights in metres above
// its lowest floor. In the level that is x = east, z = -north, y = up.
//
// The floors carry the plan. `GROUND` lists them as polygons, level or sloped. From them alone the kit
// builds, along every edge of a floor:
//   - nothing, where another floor of the same height or higher lies beyond the edge;
//   - a face down to the lower floor, where one lies beyond it (a platform's side);
//   - a wall, where nothing lies beyond it: the place is closed by the walls of its own floors.
// A floor may have a roof over it (a covered way through a block). What the floors cannot say is listed
// beside them: `WALLS` (a wall along any line, with doorways, windows and arches cut in it), `MASSES`
// (a solid of any footprint: the top of a block of houses, a pillar, a low wall), `HOUSES` (the
// reference's building at any place, height and turn, in the map's own stone), `DOORS` (two leaves of
// boards standing open), `CRATES`, `PROPS`.
//
// Every piece is made from the reference's kit: its boxes, its wall panels with real openings, its
// prisms, its buildings, its props, its collision proxies. The surfaces are the map's (./placelook.js).
// The kit also says what the walk grid needs (./mapnav.js): the undersides over floors (`ceilings`) and
// the thin walls between floors (`barriers`).
//
// One floor a place: where two floors overlap the higher one is the ground. A second level over a first
// (a room over a way, a roof a player walks on) is a floor with `over: true` and a walk grid that holds
// more than one height a node; neither is built yet, and nothing here stands in their way.
import * as THREE from 'three';
import { BOX, BOX_THIN, IDENT, LL } from '../world/kit.js';
import { fillMasks, polyPrism, solidSlabs, wallPanel } from '../world/util.js';
import { buildBuilding } from '../world/buildings.js';

const RAD = Math.PI / 180;
const _m = new THREE.Matrix4();
const _q = new THREE.Quaternion();
const UP = new THREE.Vector3(0, 1, 0);

/** Whether a point lies in a polygon of [east, north] corners. */
export function contains(at, e, n) {
  let inside = false;
  for (let i = 0, j = at.length - 1; i < at.length; j = i++) {
    const [ei, ni] = at[i], [ej, nj] = at[j];
    if (ni > n !== nj > n && e < ((ej - ei) * (n - ni)) / (nj - ni) + ei) inside = !inside;
  }
  return inside;
}

/** A floor's height at a point: its `y`, or along its slope from `y.from` to `y.to` ([east, north, up]). */
export function heightOf(g, e, n) {
  if (typeof g.y === 'number') return g.y;
  const [e0, n0, y0] = g.y.from, [e1, n1, y1] = g.y.to;
  const de = e1 - e0, dn = n1 - n0;
  const t = Math.min(1, Math.max(0, ((e - e0) * de + (n - n0) * dn) / (de * de + dn * dn)));
  return y0 + (y1 - y0) * t;
}

const highest = (g) => (typeof g.y === 'number' ? g.y : Math.max(g.y.from[2], g.y.to[2]));
/** The turn about +Y that carries +X onto a level direction. */
const turnTo = (dx, dz) => Math.atan2(-dz, dx);

/** Runs `build` with the builder's level moved by `matrix`: the reference's pieces stand where it puts them. */
function within(A, matrix, build) {
  const keep = { xform: A.xform.clone(), identity: A._identity, ry: A.ry };
  A.xform.multiply(matrix);
  A._identity = false;
  try {
    return build();
  } finally {
    A.xform.copy(keep.xform);
    A._identity = keep.identity;
    A.ry = keep.ry;
  }
}

/**
 * Builds a place. Returns `{ groundAt(e, n), ceilings, barriers, extent, floors }`: the floor's height at
 * a point (null where there is none), what the walk grid needs, the box the place fills
 * (`{ e0, n0, e1, n1, top, low }`) and the library names of the floors' surfaces.
 */
export function buildPlace(A, rng, place) {
  const plan = place.PLACE;
  const keys = { ground: 'sand', side: 'plaster_sand', wall: 'plaster_sand', band: 'concrete', cap: 'concrete', wood: 'wood', woodDark: 'wood_dark', iron: 'metal_dark', ...(place.LOOK?.keys ?? {}) };
  const grounds = place.GROUND ?? [];
  const ceilings = [], barriers = [];
  const wallH = plan.wall ?? 5.5;
  const T = 0.6;

  const groundAt = (e, n) => {
    let y = null;
    for (const g of grounds) if (contains(g.at, e, n)) y = Math.max(y ?? -Infinity, heightOf(g, e, n));
    return y;
  };
  const standAt = (e, n, y) => y ?? groundAt(e, n) ?? 0;
  const solid = (key, geo, matrix = null, masks = [0.5, 0.7, 0.4]) => {
    A.add(key, geo, matrix, { masks });
    A.collideGeo(A.surfaceOf(key), geo, matrix);
  };
  /** A box between two level points, `t` thick, from `y0` to `y1`, its middle `off` to the left of the line. */
  const run = (key, [ax, az], [bx, bz], y0, y1, t, { off = 0, more = 0, collide = true, masks = [0.5, 0.7, 0.4] } = {}) => {
    const dx = bx - ax, dz = bz - az, len = Math.hypot(dx, dz);
    if (len < 1e-3 || y1 - y0 < 1e-3) return;
    const cx = (ax + bx) / 2 + (dz / len) * off, cz = (az + bz) / 2 - (dx / len) * off;
    const ry = turnTo(dx, dz);
    A.add(key, BOX_THIN(A), LL(IDENT, cx, (y0 + y1) / 2, cz, ry, len + more, y1 - y0, t), { masks });
    if (collide) A.box(A.surfaceOf(key), cx, (y0 + y1) / 2, cz, len + more, y1 - y0, t, ry);
  };
  /** A wall's top: the band of the map's own motif and a cap over it. */
  const crown = (a, b, top, t, opts = {}) => {
    run(keys.band, a, b, top - 0.55, top - 0.05, t + 0.14, { ...opts, collide: false, masks: [0.7, 0.5, 0.25] });
    run(keys.cap, a, b, top - 0.05, top + 0.08, t + 0.26, { ...opts, collide: false, masks: [0.8, 0.4, 0.15] });
  };

  // ------------------------------------------------------------------ floors
  let e0 = Infinity, n0 = Infinity, e1 = -Infinity, n1 = -Infinity, top = 0, low = 0;
  const floors = new Set();
  for (const g of grounds) {
    const key = g.key ?? keys.ground;
    floors.add(key);
    const geo = new THREE.ShapeGeometry(new THREE.Shape(g.at.map(([e, n]) => new THREE.Vector2(e, n)))).toNonIndexed();
    geo.rotateX(-Math.PI / 2);
    const pos = geo.getAttribute('position');
    for (let i = 0; i < pos.count; i++) pos.setY(i, heightOf(g, pos.getX(i), -pos.getZ(i)));
    geo.computeVertexNormals();
    fillMasks(geo, 0.2, 0.6, 0.35);
    solid(key, geo, null, [0.2, 0.6, 0.35]);
    geo.dispose();
    for (const [e, n] of g.at) {
      e0 = Math.min(e0, e); e1 = Math.max(e1, e); n0 = Math.min(n0, n); n1 = Math.max(n1, n);
    }
    top = Math.max(top, highest(g) + (g.walls ?? wallH));
    low = Math.min(low, typeof g.y === 'number' ? g.y : Math.min(g.y.from[2], g.y.to[2]));
  }

  // ------------------------------------------------------------------ what closes a floor's edges
  for (const g of grounds) {
    let area = 0;
    for (let i = 0, j = g.at.length - 1; i < g.at.length; j = i++) area += g.at[j][0] * g.at[i][1] - g.at[i][0] * g.at[j][1];
    const side = area > 0 ? 1 : -1; // (anticlockwise: the outside is to the right of an edge)
    for (let i = 0; i < g.at.length; i++) {
      const a = g.at[i], b = g.at[(i + 1) % g.at.length];
      const de = b[0] - a[0], dn = b[1] - a[1], len = Math.hypot(de, dn);
      if (len < 0.2) continue;
      const out = [(dn / len) * side, (-de / len) * side];
      const count = Math.max(1, Math.round(len / 0.5));
      // What lies 0.3 m beyond the edge, sample by sample: 'free', 'joined', or the lower floor's height.
      const beyond = [];
      for (let k = 0; k < count; k++) {
        const t = (k + 0.5) / count;
        const pe = a[0] + de * t, pn = a[1] + dn * t;
        const qe = pe + out[0] * 0.3, qn = pn + out[1] * 0.3;
        const mine = heightOf(g, pe, pn);
        let other = contains(g.at, qe, qn) ? mine : null;
        for (const o of grounds) if (o !== g && contains(o.at, qe, qn)) other = Math.max(other ?? -Infinity, heightOf(o, qe, qn));
        beyond.push(other === null ? 'free' : other >= mine - 0.06 ? 'joined' : other);
      }
      for (let k = 0; k < count; ) {
        const kind = typeof beyond[k] === 'number' ? 'drop' : beyond[k];
        let end = k, low = Infinity;
        while (end < count && (typeof beyond[end] === 'number' ? 'drop' : beyond[end]) === kind) low = Math.min(low, typeof beyond[end] === 'number' ? beyond[end] : Infinity), end++;
        const t0 = k / count, t1 = end / count;
        const pa = [a[0] + de * t0, a[1] + dn * t0], pb = [a[0] + de * t1, a[1] + dn * t1];
        const ya = heightOf(g, pa[0], pa[1]), yb = heightOf(g, pb[0], pb[1]);
        const la = [pa[0], -pa[1]], lb = [pb[0], -pb[1]];
        if (kind === 'drop') {
          // The floor's side, down to the lower floor: two triangles that face away from the floor.
          const bottom = low - 0.25;
          const corners = side > 0 ? [la, lb] : [lb, la];
          const [p, q] = corners, yp = side > 0 ? ya : yb, yq = side > 0 ? yb : ya;
          const face = new THREE.BufferGeometry();
          face.setAttribute('position', new THREE.Float32BufferAttribute([p[0], yp, p[1], q[0], bottom, q[1], q[0], yq, q[1], p[0], yp, p[1], p[0], bottom, p[1], q[0], bottom, q[1]], 3));
          face.computeVertexNormals();
          fillMasks(face, 0.5, 0.7, 0.4);
          solid(g.side ?? keys.side, face);
          face.dispose();
        } else if (kind === 'free' && g.walls !== false) {
          // Nothing beyond: the place's own wall, standing outside the edge, with its crown.
          const wallTop = Math.max(ya, yb) + (g.walls ?? wallH);
          const more = (t0 === 0 ? T : 0) + (t1 === 1 ? T : 0);
          // (a wall is to the right of an anticlockwise edge: `run` counts to the left)
          const [p, q] = side > 0 ? [lb, la] : [la, lb];
          run(g.wall ?? keys.wall, p, q, Math.min(ya, yb) - 0.8, wallTop, T, { off: T / 2, more });
          if (g.crown !== false) crown(p, q, wallTop, T, { off: T / 2, more });
          // The windows of the houses behind the wall: small, arched, shuttered, one to so many metres.
          const length = Math.hypot(q[0] - p[0], q[1] - p[1]), every = place.LOOK?.windows?.every ?? 0;
          if (every && g.windows !== false && !g.roof && length >= every * 0.8 && wallTop - Math.max(ya, yb) >= 4) {
            const count = Math.max(1, Math.floor(length / every)), sill = Math.max(ya, yb) + 2.3;
            for (let w = 0; w < count; w++) {
              const at = ((w + 0.5) / count) * length;
              const piece = (key, from, to, y0, y1, t, masks) => run(key, [p[0] + ((q[0] - p[0]) / length) * (at + from), p[1] + ((q[1] - p[1]) / length) * (at + from)], [p[0] + ((q[0] - p[0]) / length) * (at + to), p[1] + ((q[1] - p[1]) / length) * (at + to)], y0, y1, t, { off: -t / 2, collide: false, masks });
              piece('window_void', -0.45, 0.45, sill, sill + 1.25, 0.05, [0.2, 0.9, 0.8]);
              piece('window_void', -0.3, 0.3, sill + 1.25, sill + 1.45, 0.05, [0.2, 0.9, 0.8]);
              piece(keys.woodDark, -0.92, -0.48, sill, sill + 1.25, 0.06, [0.7, 0.6, 0.3]);
              piece(keys.woodDark, 0.48, 0.92, sill, sill + 1.25, 0.06, [0.7, 0.6, 0.3]);
              piece(keys.cap, -0.6, 0.6, sill - 0.1, sill, 0.18, [0.8, 0.4, 0.15]);
            }
          }
        }
        k = end;
      }
    }
    // Steps drawn over a slope: the floor under them stays the slope a walker and the walk grid take.
    if (g.steps && typeof g.y !== 'number') {
      const [fe, fn, fy] = g.y.from, [te, tn, ty] = g.y.to;
      const le = te - fe, ln = tn - fn, length = Math.hypot(le, ln), ue = le / length, un = ln / length;
      // The floor's width across the slope's line, from its corners.
      let w0 = Infinity, w1 = -Infinity;
      for (const [e, n] of g.at) {
        const across = -(e - fe) * un + (n - fn) * ue;
        w0 = Math.min(w0, across);
        w1 = Math.max(w1, across);
      }
      const count = Math.max(2, Math.ceil(Math.abs(ty - fy) / 0.2));
      for (let k = 0; k < count; k++) {
        // (each tread is level with the slope at its upper edge)
        const s0 = (k / count) * length, s1 = ((k + 1) / count) * length;
        const yTop = fy + (ty - fy) * ((ty > fy ? k + 1 : k) / count);
        const centre = (d) => [fe + ue * d - un * ((w0 + w1) / 2), fn + un * d + ue * ((w0 + w1) / 2)];
        const [ae, an] = centre(s0), [be, bn] = centre(s1);
        run(g.key ?? keys.ground, [ae, -an], [be, -bn], yTop - 0.45, yTop, w1 - w0, { collide: false, masks: [0.6, 0.6, 0.3] });
      }
    }
    // A roof over the floor: a covered way through a block, the block's top level with the walls'.
    if (g.roof) {
      const under = highest(g) + g.roof, over = Math.max(under + 0.4, highest(g) + (g.walls ?? wallH));
      const geo = polyPrism(g.at, over - under);
      fillMasks(geo, 0.4, 0.8, 0.6);
      solid(g.roofKey ?? keys.wall, geo, _m.makeTranslation(0, under, 0), [0.4, 0.8, 0.6]);
      geo.dispose();
      ceilings.push({ poly: g.at.map(([e, n]) => [e, -n]), y: under });
    }
  }

  // ------------------------------------------------------------------ the tops of the blocks between floors
  // Where there is no floor there is a block of houses: its top is level with the nearest floor's
  // walls. Half-metre squares of the map's own plan, a strip to each run; they carry no one.
  {
    const C = 0.5, margin = 8;
    const ce0 = Math.floor(e0 - margin), cn0 = Math.floor(n0 - margin);
    const nx = Math.ceil((e1 + margin - ce0) / C), nz = Math.ceil((n1 + margin - cn0) / C);
    const lid = new Float32Array(nx * nz).fill(NaN), floor = new Uint8Array(nx * nz);
    let queue = [];
    for (let j = 0; j < nz; j++) {
      for (let i = 0; i < nx; i++) {
        const e = ce0 + (i + 0.5) * C, n = cn0 + (j + 0.5) * C;
        let best = null;
        for (const g of grounds) if (contains(g.at, e, n) && (best === null || heightOf(g, e, n) > heightOf(best, e, n))) best = g;
        if (!best) continue;
        floor[j * nx + i] = 1;
        lid[j * nx + i] = Math.round((highest(best) + (best.walls === false ? wallH : (best.walls ?? wallH))) * 2) / 2;
        queue.push(j * nx + i);
      }
    }
    while (queue.length) {
      const next = [];
      for (const c of queue) {
        const i = c % nx, j = (c / nx) | 0;
        for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const a = i + di, b = j + dj;
          if (a < 0 || b < 0 || a >= nx || b >= nz || !Number.isNaN(lid[b * nx + a])) continue;
          lid[b * nx + a] = lid[c];
          next.push(b * nx + a);
        }
      }
      queue = next;
    }
    const quads = [];
    for (let j = 0; j < nz; j++) {
      for (let i = 0; i < nx; ) {
        if (floor[j * nx + i]) {
          i++;
          continue;
        }
        let end = i;
        while (end < nx && !floor[j * nx + end] && lid[j * nx + end] === lid[j * nx + i]) end++;
        const x0 = ce0 + i * C, x1 = ce0 + end * C, z0 = -(cn0 + (j + 1) * C), z1 = -(cn0 + j * C), y = lid[j * nx + i] - 0.02;
        quads.push(x0, y, z0, x0, y, z1, x1, y, z1, x0, y, z0, x1, y, z1, x1, y, z0);
        i = end;
      }
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(quads, 3));
    geo.computeVertexNormals();
    fillMasks(geo, 0.3, 0.5, 0.3);
    A.add(keys.lid ?? keys.wall, geo, null, { masks: [0.3, 0.5, 0.3] });
    geo.dispose();
  }

  // ------------------------------------------------------------------ walls along a line
  for (const w of place.WALLS ?? []) {
    for (let s = 0; s + 1 < w.line.length; s++) {
      const [ae, an] = w.line[s], [be, bn] = w.line[s + 1];
      const a = [ae, -an], b = [be, -bn];
      const dx = b[0] - a[0], dz = b[1] - a[1], len = Math.hypot(dx, dz);
      const t = w.t ?? 0.5, sink = 0.6;
      const mid = [(ae + be) / 2, (an + bn) / 2];
      const y = w.y ?? Math.min(groundAt(mid[0] + (dz / len) * 0.6, mid[1] + (dx / len) * 0.6) ?? Infinity, groundAt(mid[0] - (dz / len) * 0.6, mid[1] - (dx / len) * 0.6) ?? Infinity);
      if (!Number.isFinite(y)) throw new Error(`WALLS: the wall from ${w.line[s]} to ${w.line[s + 1]} stands on no floor: give it a y`);
      const h = w.h ?? wallH;
      const holes = (s === 0 ? (w.holes ?? []) : []).map((o) => ({ x: o.at - len / 2, y: sink + (o.sill ?? 0) + o.h / 2, w: o.w, h: o.h, arch: o.arch ?? 0 }));
      // The panel's own space: x along the wall from its middle, y up from its foot, z through it.
      const pm = new THREE.Matrix4().compose(new THREE.Vector3((a[0] + b[0]) / 2, y - sink, (a[1] + b[1]) / 2), _q.setFromAxisAngle(UP, turnTo(dx, dz)), new THREE.Vector3(1, 1, 1)).multiply(new THREE.Matrix4().makeTranslation(0, 0, -t / 2));
      const panel = wallPanel(len, h + sink, t, holes, { bevel: 0.02 });
      fillMasks(panel, 0.5, 0.7, 0.4);
      A.add(w.key ?? keys.wall, panel, pm, { masks: [0.5, 0.7, 0.4] });
      panel.dispose();
      for (const part of solidSlabs(len, h + sink, holes)) A.slabBox(A.surfaceOf(w.key ?? keys.wall), pm, part.x, part.y, part.w, part.h, t);
      if (w.crown !== false && h > 2.2) crown(a, b, y + h, t);
      // For the walk grid: the wall closes it, but for its doorways, which are ground under a lintel.
      const along = (d) => [a[0] + (dx / len) * d, a[1] + (dz / len) * d];
      const doorways = (s === 0 ? (w.holes ?? []) : []).filter((o) => (o.sill ?? 0) < 0.45 && o.h >= 1.9).sort((p, q) => p.at - q.at);
      let from = 0;
      for (const o of doorways) {
        if (o.at - o.w / 2 > from + 0.05) barriers.push({ a: along(from), b: along(o.at - o.w / 2), t });
        const nx = (-dz / len) * (t / 2 + 0.03), nz = (dx / len) * (t / 2 + 0.03);
        const p = along(o.at - o.w / 2), q = along(o.at + o.w / 2);
        ceilings.push({ poly: [[p[0] + nx, p[1] + nz], [q[0] + nx, q[1] + nz], [q[0] - nx, q[1] - nz], [p[0] - nx, p[1] - nz]], y: y + (o.sill ?? 0) + o.h * (o.arch ? 0.8 : 1) });
        from = o.at + o.w / 2;
      }
      if (len > from + 0.05 && (w.low ?? h) > 0.5) barriers.push({ a: along(from), b: along(len), t });
    }
  }

  // ------------------------------------------------------------------ solids of any footprint
  for (const m of place.MASSES ?? []) {
    const base = m.y ?? 0;
    const geo = polyPrism(m.at, m.top - base);
    fillMasks(geo, 0.5, 0.7, 0.4);
    solid(m.key ?? keys.wall, geo, _m.makeTranslation(0, base, 0));
    geo.dispose();
    if (m.crown) for (let i = 0; i < m.at.length; i++) crown([m.at[i][0], -m.at[i][1]], [m.at[(i + 1) % m.at.length][0], -m.at[(i + 1) % m.at.length][1]], m.top, 0.1);
    top = Math.max(top, m.top);
  }

  // ------------------------------------------------------------------ the reference's houses
  let house = 0;
  for (const h of place.HOUSES ?? []) {
    const { at, size, turn = 0, y, floors = 2, ...rest } = h;
    const base = standAt(at[0], at[1], y);
    const matrix = new THREE.Matrix4().compose(new THREE.Vector3(at[0], base, -at[1]), _q.setFromAxisAngle(UP, -turn * RAD), new THREE.Vector3(1, 1, 1));
    within(A, matrix, () => buildBuilding(A, rng, { id: `H${house++}`, x: 0, z: 0, w: size[0], d: size[1], floors, wallKey: keys.wall, trimKey: keys.cap, plinthKey: keys.cap, damage: 0.15, balconies: 0, shops: false, roofProps: 0, ...rest }));
    top = Math.max(top, base + 3.45 + 3.05 * (floors - 1) + 1);
  }

  // ------------------------------------------------------------------ doors standing open
  for (const d of place.DOORS ?? []) {
    const [e, n] = d.at;
    const y = standAt(e, n, d.y), w = d.w ?? 3.2, h = d.h ?? 3.2, open = (d.open ?? 105) * RAD;
    // `facing`: the way the doorway looks, in degrees from north toward east. The leaves swing that way.
    const f = [Math.sin(d.facing * RAD), -Math.cos(d.facing * RAD)], u = [-f[1], f[0]];
    for (const s of [-1, 1]) {
      const hinge = [e + u[0] * s * (w / 2), -n + u[1] * s * (w / 2)];
      const dir = [-u[0] * s * Math.cos(open) + f[0] * Math.sin(open), -u[1] * s * Math.cos(open) + f[1] * Math.sin(open)];
      const end = [hinge[0] + dir[0] * (w / 2), hinge[1] + dir[1] * (w / 2)];
      run(keys.wood, hinge, end, y + 0.04, y + h, 0.09, { masks: [0.6, 0.6, 0.3] });
      for (const at of [0.14, 0.5, 0.86]) run(keys.iron, hinge, end, y + h * at - 0.05, y + h * at + 0.05, 0.13, { collide: false, masks: [0.9, 0.5, 0.2] });
      barriers.push({ a: hinge, b: end, t: 0.09 });
    }
  }

  // ------------------------------------------------------------------ crates of boards
  for (const c of place.CRATES ?? []) {
    const [e, n, size = 1.4, turn = 0, up] = c;
    const y = standAt(e, n, up), s = size, b = s * 0.085, at = s / 2 - b / 2;
    const pm = new THREE.Matrix4().compose(new THREE.Vector3(e, y, -n), _q.setFromAxisAngle(UP, -turn * RAD), new THREE.Vector3(1, 1, 1));
    const part = (key, x, py, z, sx, sy, sz, ry = 0, rz = 0) => A.add(key, BOX_THIN(A), LL(pm, x, py, z, ry, sx, sy, sz, 0, rz), { masks: [0.7, 0.6, 0.3] });
    A.add(keys.wood, BOX(A), LL(pm, 0, s / 2, 0, 0, s * 0.95, s * 0.95, s * 0.95), { masks: [0.6, 0.6, 0.3] });
    for (const sx of [-1, 1]) {
      for (const sz of [-1, 1]) part(keys.woodDark, sx * at, s / 2, sz * at, b, s, b);
      for (const py of [b / 2, s - b / 2]) {
        part(keys.woodDark, 0, py, sx * at, s, b, b);
        part(keys.woodDark, sx * at, py, 0, b, b, s);
      }
      // One brace across each side, from a lower corner to the upper one opposite.
      part(keys.woodDark, 0, s / 2, sx * (s / 2 - b * 0.3), s * 1.24, b * 0.9, b * 0.6, 0, sx * Math.PI / 4);
      part(keys.woodDark, sx * (s / 2 - b * 0.3), s / 2, 0, s * 1.24, b * 0.9, b * 0.6, Math.PI / 2, sx * Math.PI / 4);
    }
    A.box('wood', e, y + s / 2, -n, s, s, s, -turn * RAD);
  }

  // ------------------------------------------------------------------ the reference's props
  for (const [id, e, n, a = 0, b = 1, up] of place.PROPS ?? []) {
    const y = standAt(e, n, up);
    if (id === 'palm') {
      // (the reference's palm: its trunk, its fronds about the top, a few dead ones under them)
      const s = a || 1, ry = rng.float() * 6.28, crownY = y + 5.4 * s;
      A.put('palm_trunk', e, y, -n, ry, s, [1, rng.range(0.8, 1.2), 1]);
      const fronds = rng.int(8, 11);
      for (let i = 0; i < fronds; i++) A.putS('palm_frond', e, crownY - rng.range(0.05, 0.3), -n, ry + (i / fronds) * 6.28 + rng.range(-0.16, 0.16), s * rng.range(0.85, 1.15), s * rng.range(0.85, 1.15), s * rng.range(0.85, 1.15), [1, rng.range(0.7, 1.3), 1], 0, rng.range(-0.55, 0.15));
      for (let i = 0; i < 3; i++) A.putS('palm_frond', e, crownY - 0.35, -n, ry + rng.float() * 6.28, s * 0.8, s * 0.8, s * 0.8, [1, 1.6, 1], 0, -1.35);
      A.box('wood', e, y + 2.7 * s, -n, 0.42 * s, 5.4 * s, 0.42 * s);
    } else if (!A.has(id)) {
      throw new Error(`PROPS: the reference has no prop "${id}"`);
    } else {
      A.put(id, e, y, -n, -a * RAD, b, [1, 1.1, 1]);
    }
  }

  return { groundAt, ceilings, barriers, extent: { e0, n0, e1, n1, top, low }, floors: [...floors] };
}
