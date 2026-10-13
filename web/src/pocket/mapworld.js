// The world system of a map of our own (maps/<name>/layout.js), in place of the reference's.
//
// The reference's world is one street: its `WorldSystem` (web/src/world/index.js) builds the plan in
// `layout.js` with its kit, and keeps the spawn points, the level's place in the world and the playable
// box as constants of its own. A map of ours is a layout in the same terms (STREET, ALLEYS, BUILDINGS,
// GATE, SET_PIECES), which the export's server hands the reference's modules in place of theirs
// (web/src/pocket/vite.config.js), and three things the reference keeps to itself, which the layout
// states as `MAP`: where the level lies in the world, the spawn points, and the map's name.
//
// So this system is the reference's with another `init`: the same passes in the same order (prototypes,
// ground, buildings, gate, perimeter, dressing, debris, lights), then what the layout adds through the
// kit (walls, covered ways, raised ground, cover: ./mapkit.js). Everything else, the lights' upkeep, the
// queries, the pre-warm, is inherited. `ceilings` and `barriers` are what the export's walk grid reads
// (./mapnav.js).
import * as THREE from 'three';
import { WorldSystem } from '../world/index.js';
import { Assembler } from '../world/builder.js';
import { buildGround } from '../world/ground.js';
import { buildBuilding, collapseRoof } from '../world/buildings.js';
import { registerProps } from '../world/props.js';
import { registerDressingProps, dressStreet, dressBuildings, scatterDebris, buildGate, buildPerimeter } from '../world/dressing.js';
import * as layout from '@map/layout';
import { buildExtras } from './mapkit.js';

/** Half the side of the square the reference's perimeter wall encloses (`buildPerimeter`), metres. */
export const PERIMETER = 58;

export class MapWorldSystem extends WorldSystem {
  async init(ctx) {
    const map = layout.MAP;
    if (!map?.id || !Array.isArray(map.spawns) || map.spawns.length < 4) throw new Error('the layout states no MAP with an id and four spawn points or more');
    this.ctx = ctx;
    this.map = map;
    this.rng = ctx.rng.fork();
    const rng = this.rng;
    const materials = ctx.get('materials');
    const physics = ctx.peek('physics');
    const render = ctx.peek('render');

    this.root = new THREE.Group();
    this.root.name = 'world';
    this.root.matrixAutoUpdate = false;
    ctx.scene.add(this.root);
    materials.setGroundLevel?.(0);

    const t0 = performance.now();
    const A = new Assembler({ materials, rng, render });
    this.A = A;
    const level = { yaw: 0, tx: 0, tz: 0, ...map.level };
    A.setTransform(level.yaw, level.tx, level.tz);

    registerProps(A, rng);
    registerDressingProps(A, rng);
    buildGround(A, rng);

    const infos = [];
    for (const spec of layout.BUILDINGS) {
      const info = buildBuilding(A, rng, spec);
      infos.push(info);
      if (spec.collapse) collapseRoof(A, rng, spec, info, { x: spec.x + rng.range(-2, 2), z: spec.z + rng.range(-2, 2) });
    }
    this.buildings = infos;

    buildGate(A, rng);
    buildPerimeter(A, rng);
    // What the layout adds to the reference's plan. Its own stream of numbers: a wall more or less moves
    // nothing the reference's passes place.
    this.extras = buildExtras(A, ctx.rng.fork(), layout);
    // The undersides that stand over ground a soldier walks: the layout's own, and the gate's passage
    // (between its two blocks, under the spandrel and the walkway `buildGate` puts over it).
    const gate = layout.GATE;
    this.ceilings = [...this.extras.ceilings, { rect: [gate.xL1, gate.z - gate.depth / 2 - 0.1, gate.xR0, gate.z + gate.depth / 2 + 1.3], y: gate.height }];
    // The walls no soldier walks through: the layout's own and the perimeter's four (`buildPerimeter`).
    const R = PERIMETER;
    this.perimeter = R;
    this.barriers = [...this.extras.barriers, ...[[-R, -R, R, -R], [-R, R, R, R], [-R, -R, -R, R], [R, -R, R, R]].map(([x0, z0, x1, z1]) => ({ a: [x0, z0], b: [x1, z1], t: 0.45 }))];
    dressStreet(A, rng);
    dressBuildings(A, rng, infos);
    scatterDebris(A, rng);

    this._addLights(A);

    A.finalize(this.root, physics);
    A.releaseCache();

    this._v = new THREE.Vector3();
    this._inv = new THREE.Matrix4().copy(A.xform).invert();
    this.spawnPoints = map.spawns.map(([x, z, yaw, tag]) => ({ position: A.toWorld(x, this.extras.groundAt(x, z), z), yaw: yaw + level.yaw, tag }));
    this.bounds = new THREE.Box3(new THREE.Vector3(-PERIMETER - 4, -2, -PERIMETER - 4), new THREE.Vector3(PERIMETER + 4, 26, PERIMETER + 4)).applyMatrix4(A.xform);
    this.stats = A.stats;

    const ms = performance.now() - t0;
    console.info(
      `[world] ${map.id} built in ${ms.toFixed(0)}ms — ${(A.stats.staticTris / 1000).toFixed(0)}k static tris, ` +
        `${(A.stats.instTris / 1000).toFixed(0)}k instanced tris in ${A.stats.instances} instances, ` +
        `${A.stats.drawCalls} draw calls, ${(A.stats.collideTris / 1000).toFixed(1)}k collision tris`
    );
  }
}
