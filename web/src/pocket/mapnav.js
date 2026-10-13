// The walk grid of a map of ours: the reference's, with the ground found under what stands over it and
// with walls that close it.
//
// The reference's grid (web/src/ai/nav.js) sends one ray a cell down from above the whole level: a cell
// holds the top surface, and two cells side by side are joined when the step between them is small. On
// the reference's street that is enough. A map of ours has two things it does not hold:
//
// - Ground under a roof (a covered way), under a lintel (a doorway in a wall) and under the gate. The top
//   surface there is the roof: the cells are islands no step reaches, so no soldier would use the way.
//   For each underside the world states (`world.ceilings`: a rect in level space and the underside's
//   height), the reference's own grid class is built again over that rect with its rays started under
//   the underside, and its cells replace the main grid's cells in the rect. One more test closes a cell:
//   a ray started inside something solid (a wall that reaches the underside) leaves it through a back
//   face.
// - Walls thinner than a cell (`world.barriers`: the layout's walls and the perimeter's). A ray a cell
//   lands on such a wall here and there, the cells between stay joined across it, and a way is found
//   through the wall. Every cell whose centre is within half a cell of a wall's face is closed: a band
//   wider than a cell's diagonal, which no step crosses. The ground beyond the perimeter (`world.perimeter`,
//   half the side of its square) is closed too.
//
// The cover points are then found again on the whole grid, as the reference finds them.
import * as THREE from 'three';
import { NavGrid, CoverMap } from '../ai/nav.js';

/** Changes `ai.grid` and `ai.cover` in place. Returns the cells opened under undersides and the cells closed by walls. */
export function mapGrid(ai, world, physics) {
  const grid = ai.grid;
  const MASK = physics.MASK.WORLD;
  const corner = new THREE.Vector3(), level = new THREE.Vector3();
  let opened = 0;
  for (const ceiling of world.ceilings ?? []) {
    // An underside is a rect or a polygon of level points.
    const [x0, z0, x1, z1] = ceiling.rect ?? [0, 0, 0, 0];
    const poly = ceiling.poly ?? [[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
    const y = ceiling.y;
    const inside = (x, z) => {
      let hit = false;
      for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
        if (poly[i][1] > z !== poly[j][1] > z && x < ((poly[j][0] - poly[i][0]) * (z - poly[i][1])) / (poly[j][1] - poly[i][1]) + poly[i][0]) hit = !hit;
      }
      return hit;
    };
    // The underside's box in the world (the level is turned), a cell wider on each side.
    const bounds = new THREE.Box3();
    for (const [x, z] of poly) bounds.expandByPoint(world.levelToWorld(x, 0, z, corner));
    bounds.expandByScalar(grid.cell);
    // (NavGrid starts its rays 4 m over its bounds' top.)
    bounds.min.y = -2;
    bounds.max.y = y - 0.05 - 4;
    const under = new NavGrid(physics, { bounds, cell: grid.cell, radius: grid.radius, height: grid.height, crouchHeight: grid.crouchHeight, maxStep: grid.maxStep });
    under.build();
    for (let iz = 0; iz < under.nz; iz++) {
      for (let ix = 0; ix < under.nx; ix++) {
        const x = under.worldX(ix), z = under.worldZ(iz);
        world.worldToLevel(x, 0, z, level);
        if (!inside(level.x, level.z)) continue;
        const gx = grid.cellX(x), gz = grid.cellZ(z);
        if (!grid.inside(gx, gz)) continue;
        const from = under.index(ix, iz), to = grid.index(gx, gz);
        let flag = under.flags[from];
        if (flag) {
          const up = physics.raycast(x, under.floor[from] + 0.3, z, 0, 1, 0, 12, MASK);
          if (up.hit && !up.frontFace) flag = 0;
        }
        if (flag && (!grid.flags[to] || grid.floor[to] > under.floor[from] + 1)) opened++;
        grid.flags[to] = flag;
        grid.floor[to] = under.floor[from];
        grid.enclosure[to] = under.enclosure[from];
      }
    }
  }
  let closed = 0;
  // Beyond the perimeter wall there is ground, and no way to it.
  if (world.perimeter) {
    for (let iz = 0; iz < grid.nz; iz++) {
      for (let ix = 0; ix < grid.nx; ix++) {
        world.worldToLevel(grid.worldX(ix), 0, grid.worldZ(iz), level);
        if (Math.abs(level.x) < world.perimeter && Math.abs(level.z) < world.perimeter) continue;
        grid.flags[grid.index(ix, iz)] = 0;
      }
    }
  }
  const a = new THREE.Vector3(), b = new THREE.Vector3();
  for (const wall of world.barriers ?? []) {
    world.levelToWorld(wall.a[0], 0, wall.a[1], a);
    world.levelToWorld(wall.b[0], 0, wall.b[1], b);
    const reach = wall.t / 2 + grid.cell / 2;
    const dx = b.x - a.x, dz = b.z - a.z, len2 = dx * dx + dz * dz || 1;
    const i0 = grid.cellX(Math.min(a.x, b.x) - reach), i1 = grid.cellX(Math.max(a.x, b.x) + reach);
    const j0 = grid.cellZ(Math.min(a.z, b.z) - reach), j1 = grid.cellZ(Math.max(a.z, b.z) + reach);
    for (let iz = j0; iz <= j1; iz++) {
      for (let ix = i0; ix <= i1; ix++) {
        if (!grid.inside(ix, iz)) continue;
        const x = grid.worldX(ix), z = grid.worldZ(iz);
        const t = Math.min(1, Math.max(0, ((x - a.x) * dx + (z - a.z) * dz) / len2));
        if (Math.hypot(x - a.x - dx * t, z - a.z - dz * t) > reach) continue;
        const i = grid.index(ix, iz);
        if (grid.flags[i]) closed++;
        grid.flags[i] = 0;
      }
    }
  }
  // A doorway's middle stays open (`world.threads`): the cells within half a cell of the line through it
  // that have ground under them.
  for (const thread of world.threads ?? []) {
    world.levelToWorld(thread.a[0], 0, thread.a[1], a);
    world.levelToWorld(thread.b[0], 0, thread.b[1], b);
    const dx = b.x - a.x, dz = b.z - a.z, len2 = dx * dx + dz * dz || 1;
    for (let iz = grid.cellZ(Math.min(a.z, b.z) - 1); iz <= grid.cellZ(Math.max(a.z, b.z) + 1); iz++) {
      for (let ix = grid.cellX(Math.min(a.x, b.x) - 1); ix <= grid.cellX(Math.max(a.x, b.x) + 1); ix++) {
        if (!grid.inside(ix, iz)) continue;
        const x = grid.worldX(ix), z = grid.worldZ(iz), i = grid.index(ix, iz);
        const t = Math.min(1, Math.max(0, ((x - a.x) * dx + (z - a.z) * dz) / len2));
        if (Math.hypot(x - a.x - dx * t, z - a.z - dz * t) <= grid.cell * 0.56 && Number.isFinite(grid.floor[i]) && !grid.flags[i]) grid.flags[i] = 1;
      }
    }
  }
  // A stair steeper than a soldier's step from node to node: its open cells carry the stair's mark
  // (4 beside 1 or 2), and the cook and the match let a step between them be twice as tall.
  for (const poly of world.stairs ?? []) {
    for (let iz = 0; iz < grid.nz; iz++) {
      for (let ix = 0; ix < grid.nx; ix++) {
        const i = grid.index(ix, iz);
        if (!grid.flags[i]) continue;
        world.worldToLevel(grid.worldX(ix), 0, grid.worldZ(iz), level);
        let hit = false;
        for (let a = 0, b = poly.length - 1; a < poly.length; b = a++) {
          if (poly[a][1] > level.z !== poly[b][1] > level.z && level.x < ((poly[b][0] - poly[a][0]) * (level.z - poly[a][1])) / (poly[b][1] - poly[a][1]) + poly[a][0]) hit = !hit;
        }
        if (hit) grid.flags[i] |= 4;
      }
    }
  }
  let walk = 0;
  for (let i = 0; i < grid.flags.length; i++) if (grid.flags[i]) walk++;
  grid.walkableCount = walk;
  ai.cover = new CoverMap(grid, physics);
  ai.cover.build({ step: 1, reach: 1.3 });
  return { opened, closed };
}
