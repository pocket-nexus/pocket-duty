// The world system of a place of ours (maps/<id>/place.js): the reference's with another `init`.
//
// A place has no street, no gate and no perimeter of a fixed size, so the reference's ground, gate,
// perimeter and street dressing are not run. What is run of the reference: its prop prototypes, its
// lights' upkeep, its queries, and, piece by piece from ./placekit.js, its kit. The place states where
// it lies in the world (`PLACE.turn`: which side the sun is on), where the two sides start, its look
// (./placelook.js) and its sky.
import * as THREE from 'three';
import { WorldSystem } from '../world/index.js';
import { Assembler } from '../world/builder.js';
import { registerProps } from '../world/props.js';
import { registerDressingProps } from '../world/dressing.js';
import { BOX_THIN, IDENT, LL } from '../world/kit.js';
import * as place from '@map/place';
import { buildPlace } from './placekit.js';
import { applyLook, applySky, libraryName } from './placelook.js';
import { PALETTE } from '../world/palette.js';

// The map's surfaces are in the reference's tables before anything asks for one.
applyLook(place.LOOK);

export class PlaceWorldSystem extends WorldSystem {
  async init(ctx) {
    const plan = place.PLACE;
    if (!plan?.id || !Array.isArray(plan.starts) || plan.starts.length < 4) throw new Error('the place states no PLACE with an id and four starts or more');
    this.ctx = ctx;
    this.map = plan;
    this.rng = ctx.rng.fork();
    const rng = this.rng;
    const materials = ctx.get('materials');
    const physics = ctx.peek('physics');
    const render = ctx.peek('render');
    applySky(ctx.peek('sky'), place.LOOK);

    this.root = new THREE.Group();
    this.root.name = 'world';
    this.root.matrixAutoUpdate = false;
    ctx.scene.add(this.root);
    materials.setGroundLevel?.(0);

    const t0 = performance.now();
    const A = new Assembler({ materials, rng, render });
    this.A = A;
    A.setTransform(plan.turn ?? 0, 0, 0);
    registerProps(A, rng);
    registerDressingProps(A, rng);

    const built = buildPlace(A, rng, place);
    const { e0, n0, e1, n1, top } = built.extent;
    // Beyond the walls and under the floors: open sand, seen over a wall's top. It carries no one.
    const apron = place.LOOK?.keys?.apron ?? 'sand';
    A.add(apron, BOX_THIN(A), LL(IDENT, (e0 + e1) / 2, built.extent.low - 1.5, -(n0 + n1) / 2, 0, e1 - e0 + 160, 0.2, n1 - n0 + 160), { masks: [0.2, 0.6, 0.35] });
    this.buildings = [];
    this._addLights(A);
    A.finalize(this.root, physics);
    A.releaseCache();

    this._v = new THREE.Vector3();
    this._inv = new THREE.Matrix4().copy(A.xform).invert();
    this.place = built;
    this.ceilings = built.ceilings;
    this.barriers = built.barriers;
    // Floors at any height take the ground's shadow picture, whatever their surface (crates/duty-cook).
    this.ground = { families: [...new Set(built.floors.map((key) => PALETTE[key]?.name ?? libraryName(key)))], below: top + 1 };
    // The place's own light for the cook: a warmer sky, what its sunlit ground sends back, its lamps.
    const light = place.LOOK?.light;
    if (light || built.lamps.length) {
      this.light = { sky: light?.sky ?? [1, 1, 1], bounce: light?.bounce ?? [0, 0, 0], lamps: built.lamps.map(([x, y, z]) => ({ at: A.toWorld(x, y, z).toArray(), colour: light?.lamp ?? [9, 5.5, 2.4], reach: light?.reach ?? 11 })) };
    }
    // A start: [east, north, facing in degrees from north toward east, tag]. A yaw of 0 looks north.
    this.spawnPoints = plan.starts.map(([e, n, facing = 0, tag]) => {
      const y = built.groundAt(e, n);
      if (y === null) throw new Error(`the start "${tag}" at ${e}, ${n} stands on no floor`);
      return { position: A.toWorld(e, y, -n), yaw: -facing * (Math.PI / 180) + (plan.turn ?? 0), tag };
    });
    this.bounds = new THREE.Box3(new THREE.Vector3(e0 - 6, built.extent.low - 2, -n1 - 6), new THREE.Vector3(e1 + 6, top + 6, -n0 + 6)).applyMatrix4(A.xform);
    this.stats = A.stats;
    console.info(
      `[world] ${plan.id} built in ${(performance.now() - t0).toFixed(0)}ms: ${(A.stats.staticTris / 1000).toFixed(0)}k static tris, ` +
        `${(A.stats.instTris / 1000).toFixed(0)}k instanced tris in ${A.stats.instances} instances, ${(A.stats.collideTris / 1000).toFixed(1)}k collision tris`
    );
  }

  /** A point of the place ([east, north, up]) in the world. */
  fromPlace([e, n, up], out = new THREE.Vector3()) {
    return this.levelToWorld(e, up, -n, out);
  }
}
