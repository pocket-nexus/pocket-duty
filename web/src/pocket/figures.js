// The soldiers and the weapons in the player's hands, as the reference builds and moves them → figures.dir
// (crates/duty-cook/src/figures.rs lowers it to the pack's FIGS section, crates/duty-pack/src/figure.rs).
//
//   soldiers[]  one per variant: the skinned mesh in its bind pose (feet at the origin, facing +Z), a
//               colour before light per vertex, four bones and weights per vertex, the bones' places
//   clips[]     the soldier's clips: per frame and bone, the 3 × 4 matrix (rows) that takes a bind-pose
//               position to the posed one, sampled from the reference's animator with its foot IK off and
//               each frame stood on y = 0 by its lower sole
//   held[]      one per weapon: the weapon's meshes and both arms' in the eye's space (+X right, +Y up,
//               −Z ahead) in the hip pose, each vertex bound to one part, and its clips: per frame and
//               part the same kind of matrix, sampled from the reference's clip player
//
// Every matrix is written row by row: m[row][column], twelve floats.

const SOLDIERS = [['vanguard', 0], ['irregular', 1], ['breacher', 2]];
const SOLDIER_RATE = 20;
// `base` is the animator's locomotion clip; `speed` (m/s) is what the reference's state machine asks for in
// that gait, and with it the animator's stride rate; `aim` is the animator's aim weight; `shot` is the
// one-shot layer that runs over the base for `seconds`.
const SOLDIER_CLIPS = [
  { id: 0, name: 'AIM', base: 'idle', aim: 1, seconds: 1.5, looped: 1 },
  { id: 1, name: 'WALK_AIM', base: 'walk', aim: 1, speed: 2.0, looped: 1 },
  { id: 2, name: 'RUN', base: 'run', aim: 0, speed: 4.3, looped: 1 },
  { id: 3, name: 'PATROL', base: 'walk', aim: 0, speed: 1.35, looped: 1 },
  { id: 4, name: 'IDLE', base: 'idle', aim: 0, seconds: 1.5, looped: 1 },
  { id: 5, name: 'CROUCH_AIM', base: 'crouchIdle', crouch: true, aim: 1, seconds: 1.5, looped: 1 },
  { id: 6, name: 'CROUCH_WALK', base: 'crouchWalk', crouch: true, aim: 1, speed: 0.9, looped: 1 },
  { id: 7, name: 'FIRE', base: 'idle', aim: 1, seconds: 0.3, looped: 0, shot: 'recoil' },
  { id: 8, name: 'RELOAD', base: 'idle', aim: 1, seconds: 2.35, looped: 0, shot: 'reload' },
  { id: 9, name: 'HIT', base: 'idle', aim: 1, seconds: 0.55, looped: 0, shot: 'hit' },
];

const HELD = [['rifle', 16], ['smg', 17], ['pistol', 18]];
const HELD_RATE = 30;
const HELD_CLIPS = [['RELOAD_TAC', 1, 'reloadTac'], ['RELOAD_EMPTY', 2, 'reloadEmpty'], ['DRAW', 3, 'draw'], ['HOLSTER', 4, 'holster']];
// duty_pack::figure::part
const PART = { BODY: 0, MAGAZINE: 1, BOLT: 2, CHARGING: 3, TRIGGER: 4, UPPER_ARM_R: 5, FOREARM_R: 6, HAND_R: 7, UPPER_ARM_L: 8, FOREARM_L: 9, HAND_L: 10 };
const PARTS = 11;
// Metres a hidden magazine is put below where the clip has it.
const HIDDEN = 2;

const gamma = (c) => Math.round(Math.pow(Math.min(1, Math.max(0, c)), 1 / 2.2) * 255);

function rows(m, out, at) {
  const e = m.elements;
  for (let r = 0; r < 3; r++) for (let c = 0; c < 4; c++) out[at + r * 4 + c] = e[c * 4 + r];
}

// ---------------------------------------------------------------- the soldiers

/**
 * A tile's mean colour in linear light over a square an eighth of the tile wide around a point of it:
 * `(u, v, out)`. The tiles carry weave and grit a figure of 10 to 60 pixels cannot show; what a vertex
 * stands for is the patch around it.
 */
function tileMean(THREE, texture) {
  const { data, width: w, height: h } = texture.image;
  const srgb = texture.colorSpace === THREE.SRGBColorSpace;
  const linear = Float64Array.from({ length: 256 }, (_, i) => {
    const c = i / 255;
    return srgb ? (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)) : c;
  });
  // Sums over every rectangle that starts at the first texel.
  const W = w + 1;
  const sums = new Float64Array(W * (h + 1) * 3);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) for (let k = 0; k < 3; k++) {
    sums[((y + 1) * W + x + 1) * 3 + k] = linear[data[(y * w + x) * 4 + k]] + sums[(y * W + x + 1) * 3 + k] + sums[((y + 1) * W + x) * 3 + k] - sums[(y * W + x) * 3 + k];
  }
  const box = (x0, y0, x1, y1, k) => sums[(y1 * W + x1) * 3 + k] - sums[(y0 * W + x1) * 3 + k] - sums[(y1 * W + x0) * 3 + k] + sums[(y0 * W + x0) * 3 + k];
  const side = Math.max(1, Math.round(w / 8)), tall = Math.max(1, Math.round(h / 8));
  // The pieces of a span that runs off the tile's edge and comes back on the other side.
  const spans = (from, length, size) => {
    const a = ((from % size) + size) % size;
    return a + length <= size ? [[a, a + length]] : [[a, size], [0, a + length - size]];
  };
  return (u, v, out) => {
    u = u * texture.repeat.x + texture.offset.x;
    v = v * texture.repeat.y + texture.offset.y;
    if (texture.flipY) v = 1 - v;
    const xs = spans(Math.floor((u - Math.floor(u)) * w) - (side >> 1), side, w);
    const ys = spans(Math.floor((v - Math.floor(v)) * h) - (tall >> 1), tall, h);
    for (let k = 0; k < 3; k++) {
      let sum = 0;
      for (const [x0, x1] of xs) for (const [y0, y1] of ys) sum += box(x0, y0, x1, y1, k);
      out[k] = sum / (side * tall);
    }
  };
}

function exportSoldiers(ctx, bundle) {
  const { THREE, system, log } = ctx;
  const ai = system('ai');
  const means = new Map();
  const albedo = [0, 0, 0];

  for (const [name, model] of SOLDIERS) {
    const def = ai.variant(name);
    const g = def.geometry;
    const count = g.attributes.position.count;
    const index = Uint32Array.from(g.index.array);
    // A vertex belongs to one of the mesh's parts, and with it to one material.
    const material = new Uint8Array(count);
    for (const group of g.groups) for (let i = group.start; i < group.start + group.count; i++) material[index[i]] = group.materialIndex;
    const part = new Uint8Array(count);
    def.parts.forEach((p, k) => part.fill(k, p.start, p.start + p.count));

    // The surface's colour before light: the tile's patch × the vertex's own colour × the material's.
    const colour = new Uint8Array(count * 3);
    const uv = g.attributes.uv, tint = g.attributes.color;
    for (let i = 0; i < count; i++) {
      const m = def.materials[material[i]];
      if (m.map) {
        if (!means.has(m.map)) means.set(m.map, tileMean(THREE, m.map));
        means.get(m.map)(uv.getX(i), uv.getY(i), albedo);
      } else albedo.fill(1);
      const own = m.vertexColors && tint ? [tint.getX(i), tint.getY(i), tint.getZ(i)] : [1, 1, 1];
      colour[i * 3] = gamma(albedo[0] * own[0] * m.color.r);
      colour[i * 3 + 1] = gamma(albedo[1] * own[1] * m.color.g);
      colour[i * 3 + 2] = gamma(albedo[2] * own[2] * m.color.b);
    }

    const normal = new Int8Array(count * 3);
    const n = new THREE.Vector3();
    for (let i = 0; i < count; i++) {
      n.fromBufferAttribute(g.attributes.normal, i).normalize();
      normal.set([Math.round(n.x * 127), Math.round(n.y * 127), Math.round(n.z * 127)], i * 3);
    }
    const key = `s${model}`;
    bundle.meta.soldiers.push({
      name,
      model,
      // The variant's size against the rig the mesh, the bones and the clips are written in.
      scale: def.variant.scale ?? 1,
      vertices: count,
      triangles: index.length / 3,
      parts: def.parts.map((p) => p.name),
      materials: def.materials.map((m) => m.name),
      // The rifle's nodes in the bind pose; `muzzle` rides on HandR.
      muzzle: def.weapon.muzzle,
      weapon: Object.fromEntries(Object.entries(def.weapon).filter(([, value]) => Array.isArray(value) && value.length === 3 && typeof value[0] === 'number')),
      position: bundle.add(`${key}.position`, Float32Array.from(g.attributes.position.array)),
      normal: bundle.add(`${key}.normal`, normal),
      colour: bundle.add(`${key}.colour`, colour),
      bones: bundle.add(`${key}.bones`, Uint8Array.from(g.attributes.skinIndex.array)),
      weights: bundle.add(`${key}.weights`, Float32Array.from(g.attributes.skinWeight.array)),
      part: bundle.add(`${key}.part`, part),
      material: bundle.add(`${key}.material`, material),
      index: bundle.add(`${key}.index`, index),
    });
    log(`figures: ${name}: ${index.length / 3} triangles, ${count} vertices`);
  }

  // ---- the clips, from the reference's animator on a soldier that the AI never steps: no frame is pumped
  // while it stands in the scene, and it leaves the scene afterwards.
  const agent = ai.spawn(SOLDIERS[0][0], new THREE.Vector3(0, 0, 0), 0);
  const an = agent.animator;
  const rig = an.rig;
  an.footIk = false; // (the feet's ground rays would bend the legs to the level's floor)
  agent.group.updateMatrixWorld(true);
  const root = agent.group.matrixWorld.clone().invert();
  // Level and straight ahead, far enough that where the muzzle is makes no difference.
  const ahead = new THREE.Vector3(0, 1.4, 1e4).applyMatrix4(agent.group.matrixWorld);
  const m = new THREE.Matrix4();
  const hit = Math.min(1.4, 0.5 + system('weapons').states.get('rifle').def.damage / 45);
  const soles = ['FootR', 'ToeR', 'FootL', 'ToeL'].map((name) => rig.index(name));

  /** Poses the soldier and writes every bone's matrix in the soldier's space. */
  const sample = (clip, phase, t, out, at) => {
    // (the head looks where the rifle would aim, as it does in the reference when no target is known)
    an.setState({ clip: clip.base, speed: clip.speed ?? 0, crouch: !!clip.crouch, aimTarget: clip.aim ? ahead : null, lookTarget: ahead, aimWeight: clip.aim, suppress: 0, hurt: 0 });
    an.prevClip = clip.base;
    an.blend = 1;
    an.phase = phase;
    an.turnT = an.vaultT = -1;
    an.recoilT = clip.shot === 'recoil' ? t : -1;
    an.recoilK = 1;
    an.reloadT = clip.shot === 'reload' ? t : -1;
    an.reloadDur = clip.seconds;
    an.hitT = clip.shot === 'hit' ? t : -1;
    an.hitRegion = 'torso';
    an.hitSide = 1;
    an.hitK = hit;
    an.update(0, 0);
    agent.group.updateMatrixWorld(true);
    for (let b = 0; b < rig.count; b++) {
      m.multiplyMatrices(root, agent.bones[b].matrixWorld).multiply(agent.skeleton.boneInverses[b]);
      rows(m, out, at + b * 12);
    }
    // The animator leaves it to its foot IK to stand the soldier on the ground, and that is off here. So
    // the whole pose is raised or lowered until the lower of the two soles (under an ankle or under the
    // ball of a foot, whichever is down) is where the bind pose has it: on y = 0.
    let lift = Infinity;
    for (const b of soles) {
      const p = rig.bindPos[b], o = at + b * 12 + 4;
      lift = Math.min(lift, out[o] * p.x + out[o + 1] * p.y + out[o + 2] * p.z + out[o + 3] - p.y);
    }
    for (let b = 0; b < rig.count; b++) out[at + b * 12 + 7] -= lift;
  };
  /** Strides (or breaths) a second the animator advances `clip.base` at `clip.speed`. */
  const rateOf = (clip) => {
    an.setState({ clip: clip.base, speed: clip.speed ?? 0, crouch: !!clip.crouch, aimTarget: null, lookTarget: null, aimWeight: 0, suppress: 0, hurt: 0 });
    an.recoilT = an.hitT = an.reloadT = an.turnT = an.vaultT = -1;
    an.phase = 0;
    an.update(0.01, 0);
    return an.phase / 0.01;
  };

  for (const clip of SOLDIER_CLIPS) {
    const hz = rateOf(clip);
    let frames, rate;
    if (clip.speed) {
      // One stride, the first frame not repeated at the end; the rate that makes the frames last one
      // stride at this speed.
      frames = Math.round(SOLDIER_RATE / hz);
      rate = frames * hz;
    } else {
      // A clip that ends holds its last pose in a frame of its own.
      frames = clip.looped ? Math.round(clip.seconds * SOLDIER_RATE) : Math.ceil(clip.seconds * SOLDIER_RATE - 1e-6) + 1;
      rate = SOLDIER_RATE;
    }
    const poses = new Float32Array(frames * rig.count * 12);
    for (let f = 0; f < frames; f++) {
      const t = clip.speed ? f / rate : Math.min(f / rate, clip.seconds);
      sample(clip, clip.speed ? f / frames : (t * hz) % 1, t, poses, f * rig.count * 12);
    }
    bundle.meta.clips.push({
      id: clip.id, name: clip.name, model: SOLDIERS[0][1], bones: rig.count, frames, rate, looped: clip.looped,
      base: clip.base, aim: clip.aim, crouch: !!clip.crouch,
      // Metres a second it was sampled at and metres one pass of its frames covers at that speed.
      speed: clip.speed ?? 0, stride: clip.speed ? clip.speed / hz : 0,
      seconds: clip.speed ? 1 / hz : clip.seconds,
      poses: bundle.add(`clip${clip.id}.poses`, poses),
    });
  }
  bundle.meta.rig = {
    bones: rig.names,
    parents: rig.parent,
    // Each bone's place in the bind pose, and the far end of it.
    joints: rig.bindPos.map((p) => p.toArray()),
    tails: rig.tail.map((p) => p.toArray()),
    eyeHeight: rig.eyeHeight,
    hitStrength: hit,
  };
  ai.agents.splice(ai.agents.indexOf(agent), 1);
  agent.dispose();
  log(`figures: ${SOLDIER_CLIPS.length} clips of ${rig.count} bones: ${bundle.meta.clips.map((c) => `${c.name} ${c.frames} at ${c.rate.toFixed(2)}`).join(', ')}`);
}

// ---------------------------------------------------------------- the weapons in the hands

/** What the viewmodel's layers keep from one frame to the next, to hand back when the step is over. */
function keep(vm) {
  const springs = [vm.lag.a, vm.lag.b, vm.lag.c, vm.lagRot.a, vm.lagRot.b, vm.lagRot.c, vm.recPos.a, vm.recPos.b, vm.recPos.c, vm.recRot.a, vm.recRot.b, vm.recRot.c, vm.settle.a, vm.settle.b, vm.settle.c, vm.jumpSpring, vm.landSpring];
  const fields = ['adsT', 'adsTarget', 'sprintT', 'lowReadyT', 'triggerT', 'triggerTarget', 'bobPhase', 'noiseT', 'boltCycle', 'boltHold', 'magInHand', 'magVisible', '_hasPrev', '_prevYaw', '_prevPitch', 'clip', 'clipT', 'clipPrevT', 'onClipEvent'];
  const saved = { fields: fields.map((f) => vm[f]), springs: springs.map((s) => [s.x, s.v, s.target]), yaw: vm._angVel.yaw, pitch: vm._angVel.pitch };
  return () => {
    fields.forEach((f, i) => { vm[f] = saved.fields[i]; });
    springs.forEach((s, i) => { [s.x, s.v, s.target] = saved.springs[i]; });
    vm._angVel.yaw = saved.yaw;
    vm._angVel.pitch = saved.pitch;
  };
}

function exportHeld(ctx, bundle) {
  const { THREE, system, log } = ctx;
  const weapons = system('weapons');
  const vm = weapons.viewmodel;
  const vcam = ctx.engine.viewCamera;
  const restore = keep(vm);
  const active = weapons.activeId, fov = vcam.fov;

  // Everything the viewmodel adds to the base pose, at zero: the lag, recoil, settle, jump and landing
  // springs at rest with no speed, no camera turn to chase, no aim, sprint or lowered pose blended in, the
  // bolt forward and the trigger out. Sway and breath are scaled by the weapon's `swayScale`, which is set
  // to zero for the step; bob is scaled by the speed handed to `update`, which is zero.
  const still = () => {
    vm.stopClip();
    vm.adsT = vm.sprintT = vm.lowReadyT = vm.triggerT = 0;
    vm.boltCycle = vm.boltHold = 0;
    vm.lag.reset(); vm.lagRot.reset(); vm.recPos.reset(); vm.recRot.reset(); vm.settle.reset();
    vm.jumpSpring.set(0); vm.landSpring.set(0);
    vm._hasPrev = false;
    vm._angVel.yaw = vm._angVel.pitch = 0;
  };
  const STILL = { ads: false, sprint: false, lowReady: false, speed: 0, crouch: false, airborne: false, trigger: false, empty: false };
  const m = new THREE.Matrix4(), normalMatrix = new THREE.Matrix3();
  const v = new THREE.Vector3();
  // The viewmodel hangs on an anchor that follows the camera. For the step the anchor stands still, level,
  // at the height of an eye, so what the surface program is asked does not depend on where the camera has
  // got to; and a node's place in the eye's space is read from the anchor down, never through the world.
  const tracked = vm.trackCamera, anchorAt = vm.anchor.position.clone(), anchorTurn = vm.anchor.quaternion.clone();
  vm.trackCamera = false;
  vm.anchor.position.set(0, 1.6, 0);
  vm.anchor.quaternion.identity();
  const inEye = (node, out = new THREE.Matrix4()) => {
    out.copy(node.matrix);
    for (let p = node.parent; p && p !== vm.anchor; p = p.parent) out.premultiply(p.matrix);
    return out;
  };
  /** One step of the viewmodel with no time passing. */
  const pose = (state = STILL) => {
    vm.update(0, state);
    vm.anchor.updateMatrixWorld(true);
  };

  for (const [id, model] of HELD) {
    weapons.setWeaponImmediate(id);
    const w = vm.active, def = w.def;
    const sway = def.swayScale;
    def.swayScale = 0;
    still();
    pose();
    // The hip pose and nothing else.
    const hip = new THREE.Matrix4().compose(new THREE.Vector3().fromArray(def.hipPos), new THREE.Quaternion().setFromEuler(new THREE.Euler(def.hipRot[0], def.hipRot[1], def.hipRot[2], 'XYZ')), new THREE.Vector3(1, 1, 1));
    const off = Math.max(...vm.rig.matrix.elements.map((e, i) => Math.abs(e - hip.elements[i])));
    if (off > 1e-9) throw new Error(`figures: ${id} does not rest in its hip pose (off by ${off})`);

    // The node that carries each part. A part the weapon lacks goes with the body.
    const nodes = new Array(PARTS).fill(w.group);
    nodes[PART.MAGAZINE] = w.parts.magazine ?? w.group;
    nodes[PART.BOLT] = w.parts.bolt ?? w.parts.slide ?? w.group;
    nodes[PART.CHARGING] = w.parts.charging ?? w.group;
    nodes[PART.TRIGGER] = w.parts.trigger ?? w.group;
    [[vm.armR, PART.UPPER_ARM_R], [vm.armL, PART.UPPER_ARM_L]].forEach(([arm, first]) => {
      nodes[first] = arm.upperPivot;
      nodes[first + 1] = arm.forePivot;
      nodes[first + 2] = arm.hand;
    });
    const partOf = (mesh) => {
      for (let o = mesh; o; o = o.parent) {
        // (the selector never turns: it is the body's)
        const at = nodes.indexOf(o);
        if (at >= 0) return o === w.group ? PART.BODY : at;
      }
      throw new Error(`figures: ${mesh.name} hangs on no part`);
    };
    const bindInverse = nodes.map((node) => inEye(node).invert());

    // ---- the meshes, in the eye's space as they stand now
    const meshes = [...w.meshes];
    for (const arm of [vm.armR, vm.armL]) arm.root.traverse((o) => { if (o.isMesh) meshes.push(o); });
    const records = [];
    let vertices = 0, indices = 0, flat = 0, skipped = 0;
    for (const mesh of meshes) {
      const material = mesh.material;
      // Glass, the lens' ring and its vignette are drawn over what is behind them: not a surface's colour.
      if (material.transparent || material.blending !== THREE.NormalBlending) { skipped++; continue; }
      records.push({ mesh, first: vertices, start: indices });
      vertices += mesh.geometry.attributes.position.count;
      indices += mesh.geometry.index ? mesh.geometry.index.count : mesh.geometry.attributes.position.count;
    }
    const position = new Float32Array(vertices * 3), normal = new Int8Array(vertices * 3), colour = new Uint8Array(vertices * 3), part = new Uint8Array(vertices), index = new Uint32Array(indices);
    const list = [];
    for (const { mesh, first, start } of records) {
      const g = mesh.geometry, material = mesh.material;
      const count = g.attributes.position.count;
      inEye(mesh, m);
      normalMatrix.getNormalMatrix(m);
      for (let i = 0; i < count; i++) {
        v.fromBufferAttribute(g.attributes.position, i).applyMatrix4(m);
        position.set([v.x, v.y, v.z], (first + i) * 3);
        v.fromBufferAttribute(g.attributes.normal, i).applyMatrix3(normalMatrix).normalize();
        normal.set([Math.round(v.x * 127), Math.round(v.y * 127), Math.round(v.z * 127)], (first + i) * 3);
      }
      const p = partOf(mesh);
      part.fill(p, first, first + count);
      // A mirrored mesh (the right hand is the left one's mirror) shows its faces the other way round.
      const mirrored = m.determinant() < 0;
      const n = g.index ? g.index.count : count;
      for (let i = 0; i < n; i += 3) {
        const at = (k) => first + (g.index ? g.index.getX(i + k) : i + k);
        index.set(mirrored ? [at(0), at(2), at(1)] : [at(0), at(1), at(2)], start + i);
      }
      const library = !!(material.map && material.userData?.owParams);
      if (library) colour.set(ctx.bakeVertices(mesh).colour, first * 3);
      else {
        // The reference's own flat materials (the black of a bore, the inside of the optic's tube).
        const c = [gamma(material.color.r), gamma(material.color.g), gamma(material.color.b)];
        for (let i = 0; i < count; i++) colour.set(c, (first + i) * 3);
        flat++;
      }
      list.push({ name: mesh.name || mesh.parent?.name || 'mesh', material: material.name.split('|')[0], part: p, first, vertices: count, start, indices: n, doubleSided: material.side === THREE.DoubleSide, library });
    }

    // ---- the points and numbers the simulation needs
    const body = inEye(w.group);
    const point = (p) => p.clone().applyMatrix4(body).toArray();
    const direction = (d) => d.clone().transformDirection(body).toArray();
    const travel = w.parts.slide ? w.slideTravel : w.boltTravel;
    const hipMatrix = vm.rig.matrix.clone();
    // The pose for aiming, from the viewmodel itself: fully aimed, nothing else changed.
    vm.adsT = 1;
    pose({ ...STILL, ads: true });
    const adsMatrix = vm.rig.matrix.clone();
    const adsFov = vcam.fov;
    const carry = new THREE.Matrix4().multiplyMatrices(adsMatrix, hipMatrix.clone().invert());
    const turn = new THREE.Euler().setFromRotationMatrix(carry, 'XYZ');
    // The same for the two other poses the viewmodel blends the hip pose with: sprinting and lowered.
    const carried = (prepare, state) => {
      still();
      prepare();
      pose({ ...STILL, ...state });
      const c = new THREE.Matrix4().multiplyMatrices(vm.rig.matrix, hipMatrix.clone().invert());
      const e = new THREE.Euler().setFromRotationMatrix(c, 'XYZ');
      return { offset: [c.elements[12], c.elements[13], c.elements[14]], turn: [e.x, e.y, e.z] };
    };
    const sprint = carried(() => { vm.sprintT = 1; }, { sprint: true });
    const lowered = carried(() => { vm.lowReadyT = 1; }, { lowReady: true });
    still();
    pose();
    const key = `h${model}`;
    const record = {
      name: id,
      model,
      vertices,
      triangles: indices / 3,
      meshes: list,
      position: bundle.add(`${key}.position`, position),
      normal: bundle.add(`${key}.normal`, normal),
      colour: bundle.add(`${key}.colour`, colour),
      part: bundle.add(`${key}.part`, part),
      index: bundle.add(`${key}.index`, index),
      // In the eye's space, in the hip pose.
      muzzle: point(w.muzzle),
      eject: point(w.eject),
      ejectDir: direction(w.ejectDir),
      sight: point(w.sight),
      // The weapon's own axes there: +X its right, +Y its top, +Z from the muzzle to the stock.
      axes: [direction(new THREE.Vector3(1, 0, 0)), direction(new THREE.Vector3(0, 1, 0)), direction(new THREE.Vector3(0, 0, 1))],
      boltTravel: travel.z,
      triggerPivot: w.parts.trigger ? new THREE.Vector3().setFromMatrixPosition(inEye(w.parts.trigger, m)).toArray() : null,
      triggerPull: w.triggerPull,
      // A part's place when aiming is `turn` (Euler XYZ as three.js composes it: x, then y, then z, about
      // the eye) applied to its place in the hip pose, then moved by `offset`.
      adsOffset: [carry.elements[12], carry.elements[13], carry.elements[14]],
      adsTurn: [turn.x, turn.y, turn.z],
      hip: { position: def.hipPos, rotation: def.hipRot, fov },
      ads: { position: new THREE.Vector3().setFromMatrixPosition(adsMatrix).toArray(), rotation: new THREE.Euler().setFromRotationMatrix(adsMatrix, 'XYZ').toArray().slice(0, 3), fov: adsFov, seconds: def.adsTime },
      sprint,
      lowered,
      clips: [],
    };

    // ---- the clips, from the viewmodel's own player with the layers still at zero
    const rest = new Float32Array(PARTS * 12);
    for (let p = 0; p < PARTS; p++) rows(m.identity(), rest, p * 12);
    record.clips.push({ id: 32 + (model - 16) * 5, name: 'REST', frames: 1, rate: HELD_RATE, looped: 1, seconds: 0, events: [], poses: bundle.add(`${key}.clip0`, rest) });
    for (const [label, which, name] of HELD_CLIPS) {
      const clip = w.clips[name];
      // The last frame is the clip's end.
      const frames = Math.ceil(clip.duration * HELD_RATE - 1e-6) + 1;
      const poses = new Float32Array(frames * PARTS * 12);
      still();
      // A magazine run dry leaves the bolt back until the clip lets it go.
      vm.boltHold = name === 'reloadEmpty' ? 1 : 0;
      vm.onClipEvent = (event) => { if (event === 'boltrelease') vm.boltHold = 0; };
      vm.play(name);
      let hidden = 0;
      for (let f = 0; f < frames; f++) {
        vm.clipT = Math.min(f / HELD_RATE, clip.duration);
        pose();
        for (let p = 0; p < PARTS; p++) {
          inEye(nodes[p], m).multiply(bindInverse[p]);
          if (p === PART.MAGAZINE && !nodes[p].visible) {
            m.elements[13] -= HIDDEN;
            hidden++;
          }
          rows(m, poses, (f * PARTS + p) * 12);
        }
      }
      vm.stopClip();
      record.clips.push({
        id: 32 + (model - 16) * 5 + which, name: label, source: name, frames, rate: HELD_RATE, looped: 0, seconds: clip.duration,
        events: clip.events.map((e) => ({ t: e.t, name: e.name })),
        hiddenFrames: hidden,
        poses: bundle.add(`${key}.clip${which}`, poses),
      });
    }
    w.parts.magazine.visible = true;
    def.swayScale = sway;
    bundle.meta.held.push(record);
    log(`figures: ${id}: ${indices / 3} triangles in ${list.length} meshes (${flat} of the reference's flat materials, ${skipped} see-through ones left out); ${record.clips.map((c) => `${c.name} ${c.frames}`).join(', ')}`);
  }

  weapons.setWeaponImmediate(active);
  restore();
  vm.trackCamera = tracked;
  vm.anchor.position.copy(anchorAt);
  vm.anchor.quaternion.copy(anchorTurn);
  vcam.fov = fov;
  vcam.updateProjectionMatrix();
}

export async function exportFigures(ctx) {
  const bundle = new ctx.Bundle({ soldiers: [], clips: [], rig: null, held: [], parts: Object.keys(PART), soldierRate: SOLDIER_RATE, heldRate: HELD_RATE });
  exportSoldiers(ctx, bundle);
  exportHeld(ctx, bundle);
  await ctx.put('figures.dir', bundle.blob());
  ctx.log(`figures: ${bundle.meta.soldiers.length} soldiers, ${bundle.meta.clips.length} clips, ${bundle.meta.held.length} held weapons, ${(bundle.bytes / 1e6).toFixed(1)} MB`);
}
