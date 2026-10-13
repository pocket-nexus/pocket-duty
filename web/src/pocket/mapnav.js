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
  for (const { rect: [x0, z0, x1, z1], y } of world.ceilings ?? []) {
    // The rect's box in the world (the level is turned), a cell wider on each side.
    const bounds = new THREE.Box3();
    for (const [x, z] of [[x0, z0], [x1, z0], [x0, z1], [x1, z1]]) bounds.expandByPoint(world.levelToWorld(x, 0, z, corner));
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
        if (level.x < x0 || level.x > x1 || level.z < z0 || level.z > z1) continue;
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
  let walk = 0;
  for (let i = 0; i < grid.flags.length; i++) if (grid.flags[i]) walk++;
  grid.walkableCount = walk;
  ai.cover = new CoverMap(grid, physics);
  ai.cover.build({ step: 1, reach: 1.3 });
  return { opened, closed };
}
