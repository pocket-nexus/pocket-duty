// DutyIR export. The page starts the reference with its capture seed, lets it build everything it builds at
// load, and hands what it produced to the compiler (tools/export.ts receives it):
//
//   world.dir      every world mesh as the reference draws it (positions, normals, instances), with what the
//                  reference's own surface program gives each vertex: its colour before light, and the light
//                  it sends back with the sun switched off (the sky's fill, the ground's, the environment)
//   tone.dir       the reference's own display transform (exposure, tone curve, grade) as a table
//   tex_<n>.png    the forged albedo of each surface family
//   collision.dir  the physics system's triangles and their surface types
//   nav.dir        the AI's walk grid, its cover points and the spawn points
//   sky.dir        the sky as six HDR faces, the sun and the lamps
//   figures.dir    the soldiers and the held weapons: meshes, bones, sampled clips (figures.js)
//   audio.dir      every sound, rendered by the reference's synthesis (audio.js)
//   scene.json     what the other files need to be read: bounds, spawn points, the level's axes
//
//   export.html?sink=http://127.0.0.1:5291[&only=world,collision,…][&map=<name>]
//
// `map` is a map of ours: the server was started for it (tools/export.ts), and the world is built from
// its layout (./mapworld.js). `only=look` exports nothing and leaves the page to tools/export.ts, which
// poses the camera (`window.__LOOK__`) and takes pictures of the map as the reference draws it.
import * as THREE from 'three';
import { boot } from './boot.js';
import { Bundle } from './bundle.js';
import { mapGrid } from './mapnav.js';
import { exportFigures } from './figures.js';
import { exportAudio } from './audio.js';

const params = new URLSearchParams(location.search);
const SINK = params.get('sink');
const ONLY = params.get('only')?.split(',');
const MAP = params.get('map') ?? '';
const wants = (part) => !ONLY || ONLY.includes(part);

const logEl = document.getElementById('log');
const log = (...a) => { const line = a.join(' '); console.log(line); logEl.textContent = line + '\n' + logEl.textContent.slice(0, 4000); };
const put = async (name, body) => {
  if (!SINK) return;
  const res = await fetch(`${SINK}/${name}`, { method: 'PUT', body });
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}`);
};

// The lowest preset: its surface programs read nothing from the screen (no ambient occlusion buffer, no
// contact shadows, no reflections), so a vertex drawn as a point gets what the surface alone decides.
const { engine, pump, system } = await boot({ quality: params.get('q') ?? 'low', map: MAP });
const renderer = system('render').renderer;
log('reference started');

// PNG of RGBA rows that start at the top.
async function png(pixels, w, h) {
  const canvas = new OffscreenCanvas(w, h), g = canvas.getContext('2d');
  g.putImageData(new ImageData(new Uint8ClampedArray(pixels.buffer, pixels.byteOffset, pixels.byteLength), w, h), 0, 0);
  return canvas.convertToBlob({ type: 'image/png' });
}

const srgbToLinear = (c) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
const linearToSrgb = (c) => (c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055);

// ---------------------------------------------------------------- pictures
const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), new THREE.ShaderMaterial({
  uniforms: { map: { value: null } },
  vertexShader: 'varying vec2 vUv; void main() { vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }',
  // (a texture marked sRGB is decoded when it is sampled)
  fragmentShader: 'uniform sampler2D map; varying vec2 vUv; void main() { vec4 t = texture2D(map, vUv); gl_FragColor = vec4(pow(max(t.rgb, 0.0), vec3(1.0 / 2.2)), t.a); }',
  depthTest: false, depthWrite: false,
}));
quad.frustumCulled = false;
const quadScene = new THREE.Scene();
quadScene.add(quad);
const flatCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

/** A texture's texels as the reference samples them: RGBA bytes, top row first, colour at gamma 2.2. */
function readTexture(texture) {
  const w = texture.image.width, h = texture.image.height;
  const target = new THREE.WebGLRenderTarget(w, h, { depthBuffer: false, minFilter: THREE.NearestFilter, magFilter: THREE.NearestFilter });
  quad.material.uniforms.map.value = texture;
  const saved = renderer.getRenderTarget();
  renderer.setRenderTarget(target);
  renderer.render(quadScene, flatCamera);
  const raw = new Uint8Array(w * h * 4);
  renderer.readRenderTargetPixels(target, 0, 0, w, h, raw);
  renderer.setRenderTarget(saved);
  target.dispose();
  const out = new Uint8Array(raw.length);
  for (let y = 0; y < h; y++) out.set(raw.subarray((h - 1 - y) * w * 4, (h - y) * w * 4), y * w * 4);
  return { pixels: out, width: w, height: h };
}

// ---------------------------------------------------------------- the colour of a vertex
// The reference colours a surface in its fragment program: the forged tile, then tint, dust on what faces up,
// runoff under sills, the splash band at a wall's foot, grime in the masks' cavities, wear on the masks'
// edges. The handhelds draw the tile times a colour per vertex, so the export asks the reference's own program
// for that colour: each vertex is drawn as one point at a texel of its own, with the surface's tile replaced by
// its mean colour, and the program writes its albedo instead of the lit result.
const BAKE = { value: new THREE.Vector4(1, 1, 1, 0) };
const BAKE_W = 2048;
const prepared = new Map();

function prepare(material) {
  if (prepared.has(material)) return prepared.get(material);
  const info = { mean: [0.5, 0.5, 0.5], baked: false };
  prepared.set(material, info);
  if (!material.map || !material.userData?.owParams) return info;
  const { pixels, width, height } = readTexture(material.map);
  // The mean in linear light, where the program mixes.
  let r = 0, g = 0, b = 0, a = 0;
  for (let i = 0; i < pixels.length; i += 4) {
    r += Math.pow(pixels[i] / 255, 2.2); g += Math.pow(pixels[i + 1] / 255, 2.2); b += Math.pow(pixels[i + 2] / 255, 2.2); a += pixels[i + 3];
  }
  const n = width * height;
  info.mean = [r / n, g / n, b / n];
  info.texture = { pixels, width, height };
  if (material.userData.owParams.alphaMask) return info; // a cut-out keeps its tile's own colours
  const flat = new THREE.DataTexture(new Uint8Array([...info.mean.map((c) => Math.round(linearToSrgb(c) * 255)), Math.round(a / n)]), 1, 1);
  flat.colorSpace = THREE.SRGBColorSpace;
  flat.wrapS = flat.wrapT = THREE.RepeatWrapping;
  flat.needsUpdate = true;
  material.map = flat;
  const compile = material.onBeforeCompile;
  material.onBeforeCompile = (shader, r3) => {
    compile.call(material, shader, r3);
    shader.uniforms.pkBake = BAKE;
    shader.vertexShader = 'uniform vec4 pkBake;\n' + shader.vertexShader.replace(/}\s*$/, `
  {
    int pkI = gl_VertexID + gl_InstanceID * int(pkBake.z);
    int pkW = int(pkBake.x);
    gl_Position = vec4((float(pkI % pkW) + 0.5) / pkBake.x * 2.0 - 1.0, (float(pkI / pkW) + 0.5) / pkBake.y * 2.0 - 1.0, 0.0, 1.0);
    gl_PointSize = 1.0;
  }
}`);
    // pkBake.w: 0 writes the surface's colour before light, 1 what the program made of it and the light.
    shader.fragmentShader = 'uniform vec4 pkBake;\n' + shader.fragmentShader.replace(/}\s*$/, `
  gl_FragColor = vec4(pkBake.w > 0.5 ? max(gl_FragColor.rgb, 0.0) : max(owAlbedo.rgb, 0.0), 1.0);
}`);
  };
  const key = material.customProgramCacheKey.bind(material);
  material.customProgramCacheKey = () => key() + '|pkbake';
  material.needsUpdate = true;
  info.baked = true;
  return info;
}

// No lamp and no sun stand in this scene: what a surface sends back here is what the reference's fills and
// its environment give it.
const bakeScene = new THREE.Scene();
bakeScene.environment = engine.scene.environment;
bakeScene.environmentIntensity = engine.scene.environmentIntensity;
// Far from everything: the tile's own detail fades with distance, and the tile is the device's to draw.
const bakeCamera = new THREE.PerspectiveCamera(60, 1, 1, 100000);
bakeCamera.position.set(0, 40000, 0);
bakeCamera.updateMatrixWorld(true);

/**
 * What the reference's surface program gives every vertex of `mesh` (of every instance, instance by
 * instance): `colour`, its albedo as RGB bytes at gamma 2.2, and `ambient`, the linear light it sends back
 * with no sun, three floats.
 */
function bakeVertices(mesh) {
  const info = prepare(mesh.material);
  const verts = mesh.geometry.attributes.position.count;
  const instances = mesh.isInstancedMesh ? mesh.count : 1;
  const total = verts * instances;
  const colour = new Uint8Array(total * 3);
  const ambient = new Float32Array(total * 3);
  if (!info.baked) {
    const flat = info.mean.map((c) => Math.round(Math.pow(c, 1 / 2.2) * 255));
    for (let i = 0; i < total; i++) colour.set(flat, i * 3);
    ambient.fill(-1); // (the compiler takes the level's mean for a surface the program could not be asked)
    return { colour, ambient };
  }
  const h = Math.ceil(total / BAKE_W);
  const target = new THREE.WebGLRenderTarget(BAKE_W, h, { type: THREE.FloatType, depthBuffer: false, minFilter: THREE.NearestFilter, magFilter: THREE.NearestFilter });
  // The same geometry and instances, drawn as points.
  const points = mesh.isInstancedMesh ? new THREE.InstancedMesh(mesh.geometry, mesh.material, mesh.count) : new THREE.Mesh(mesh.geometry, mesh.material);
  if (mesh.isInstancedMesh) {
    points.instanceMatrix = mesh.instanceMatrix;
    points.instanceColor = mesh.instanceColor;
  }
  points.matrixAutoUpdate = false;
  points.matrix.copy(mesh.matrixWorld);
  points.matrixWorld.copy(mesh.matrixWorld);
  points.isMesh = false;
  points.isPoints = true;
  points.frustumCulled = false;
  bakeScene.add(points);
  const saved = renderer.getRenderTarget();
  const raw = new Float32Array(BAKE_W * h * 4);
  let missed = 0;
  for (const mode of [0, 1]) {
    BAKE.value.set(BAKE_W, h, verts, mode);
    renderer.setRenderTarget(target);
    renderer.setClearColor(0x000000, 0);
    renderer.clear(true, false, false);
    renderer.render(bakeScene, bakeCamera);
    renderer.readRenderTargetPixels(target, 0, 0, BAKE_W, h, raw);
    for (let i = 0; i < total; i++) {
      if (mode === 0) {
        if (raw[i * 4 + 3] === 0) missed++;
        for (let k = 0; k < 3; k++) colour[i * 3 + k] = Math.round(Math.pow(Math.min(1, raw[i * 4 + k]), 1 / 2.2) * 255);
      } else for (let k = 0; k < 3; k++) ambient[i * 3 + k] = raw[i * 4 + k];
    }
  }
  renderer.setRenderTarget(saved);
  bakeScene.remove(points);
  target.dispose();
  if (missed) log(`  ${mesh.name}: ${missed} of ${total} vertices were not drawn`);
  return { colour, ambient };
}

// ---------------------------------------------------------------- the world
async function exportWorld() {
  const world = system('world');
  const bundle = new Bundle({ meshes: [], materials: [] });
  const materialIds = new Map();
  const textures = new Map();
  const materialOf = (m) => {
    if (materialIds.has(m)) return materialIds.get(m);
    const p = m.userData?.owParams ?? {};
    const info = prepare(m);
    const family = m.name.split('|')[0] || 'plain';
    if (info.texture && !textures.has(family)) textures.set(family, info.texture);
    const id = bundle.meta.materials.length;
    bundle.meta.materials.push({
      name: m.name,
      family,
      texture: info.texture ? `tex_${family}.png` : null,
      // Metres a tile covers, and how the reference projects it (`planar`: along the dominant axis of the
      // normal in world space; `triplanar`: the three projections mixed; `mesh`: the mesh's own coordinates).
      projection: p.uvMode ?? 'planar',
      scale: p.scale ?? 1,
      offset: p.offset ?? [0, 0],
      mean: info.mean,
      doubleSided: m.side === THREE.DoubleSide,
      transparent: !!m.transparent,
      opacity: m.opacity,
      cutout: !!p.alphaMask,
      alphaTest: m.alphaTest ?? 0,
      emissive: m.emissive ? m.emissive.toArray().map((c) => c * (m.emissiveIntensity ?? 1)) : [0, 0, 0],
    });
    materialIds.set(m, id);
    return id;
  };
  let tris = 0, vertsTotal = 0;
  const v = new THREE.Vector3();
  const normalMatrix = new THREE.Matrix3();
  for (const [i, mesh] of world.A.meshes.entries()) {
    const g = mesh.geometry;
    const count = g.attributes.position.count;
    const name = `m${i}`;
    // Positions and normals in world space for what is drawn once; a prototype stays in its own space, with
    // the matrix of each instance.
    const pos = new Float32Array(count * 3), nrm = new Int8Array(count * 3);
    normalMatrix.getNormalMatrix(mesh.matrixWorld);
    for (let k = 0; k < count; k++) {
      v.fromBufferAttribute(g.attributes.position, k);
      if (!mesh.isInstancedMesh) v.applyMatrix4(mesh.matrixWorld);
      pos.set([v.x, v.y, v.z], k * 3);
      if (g.attributes.normal) {
        v.fromBufferAttribute(g.attributes.normal, k);
        if (!mesh.isInstancedMesh) v.applyMatrix3(normalMatrix);
        v.normalize();
        nrm.set([Math.round(v.x * 127), Math.round(v.y * 127), Math.round(v.z * 127)], k * 3);
      }
    }
    const index = g.index ? Uint32Array.from(g.index.array) : Uint32Array.from({ length: count }, (_, k) => k);
    const record = {
      name: mesh.name,
      material: materialOf(mesh.material),
      surface: mesh.userData.surface ?? null,
      // Metres from the eye beyond which the reference stops drawing a small prop.
      reach: mesh.userData.owLodDist ?? null,
      vertices: count,
      triangles: index.length / 3,
      instances: mesh.isInstancedMesh ? mesh.count : 0,
      hasNormals: !!g.attributes.normal,
      position: bundle.add(`${name}.position`, pos),
      normal: bundle.add(`${name}.normal`, nrm),
      index: bundle.add(`${name}.index`, index),
    };
    const baked = bakeVertices(mesh);
    record.colour = bundle.add(`${name}.colour`, baked.colour);
    record.ambient = bundle.add(`${name}.ambient`, baked.ambient);
    if (mesh.material.userData?.owParams?.uvMode === 'mesh' && g.attributes.uv) record.uv = bundle.add(`${name}.uv`, Float32Array.from(g.attributes.uv.array));
    if (mesh.isInstancedMesh) {
      const matrices = new Float32Array(mesh.count * 16);
      const m4 = new THREE.Matrix4();
      for (let k = 0; k < mesh.count; k++) {
        mesh.getMatrixAt(k, m4);
        m4.premultiply(mesh.matrixWorld);
        matrices.set(m4.elements, k * 16);
      }
      record.matrices = bundle.add(`${name}.matrices`, matrices);
    }
    bundle.meta.meshes.push(record);
    tris += record.triangles * Math.max(1, record.instances);
    vertsTotal += count * Math.max(1, record.instances);
    if (i % 24 === 0) {
      log(`world: ${i + 1} of ${world.A.meshes.length} meshes`);
      await new Promise((r) => setTimeout(r));
    }
  }
  for (const [family, t] of textures) await put(`tex_${family}.png`, await png(t.pixels, t.width, t.height));
  await put('world.dir', bundle.blob());
  log(`world: ${bundle.meta.meshes.length} meshes, ${bundle.meta.materials.length} materials, ${textures.size} tiles, ${tris} triangles, ${vertsTotal} coloured vertices, ${(bundle.bytes / 1e6).toFixed(1)} MB`);
}

// ---------------------------------------------------------------- collision
async function exportCollision() {
  const physics = system('physics');
  const w = physics.staticWorld;
  const n = w.triCount;
  const bundle = new Bundle({ triangles: n, surfaces: physics.SURFACE_NAMES, properties: physics.SURFACE_PROPS, layers: physics.LAYER, masks: physics.MASK, gravity: physics.gravity });
  bundle.add('position', Float32Array.from(w.pos.subarray(0, n * 9)));
  bundle.add('surface', Uint8Array.from(w.surface.subarray(0, n)));
  bundle.add('mask', Uint16Array.from(w.mask.subarray(0, n)));
  await put('collision.dir', bundle.blob());
  log(`collision: ${n} triangles`);
}

// ---------------------------------------------------------------- where the soldiers walk
async function exportNav() {
  const ai = system('ai');
  const world = system('world');
  // A map of ours has ground under roofs, lintels and the gate, and walls thinner than a cell (./mapnav.js).
  if (world.map) {
    const { opened, closed } = mapGrid(ai, world, system('physics'));
    log(`nav: ${opened} cells opened under ${world.ceilings.length} roofs, lintels and gates, ${closed} closed by ${world.barriers.length} walls`);
  }
  const grid = ai.grid;
  const bundle = new Bundle({
    cell: grid.cell, minX: grid.minX, minZ: grid.minZ, nx: grid.nx, nz: grid.nz,
    radius: grid.radius, height: grid.height, crouchHeight: grid.crouchHeight, maxStep: grid.maxStep,
    walkable: grid.walkableCount,
    cover: ai.cover.points.length,
    spawns: world.spawnPoints.map((s) => ({ position: s.position.toArray(), yaw: s.yaw, tag: s.tag })),
  });
  // Per cell: 0 closed, 1 a soldier stands, 2 a soldier crouches; and the floor's height there.
  bundle.add('flags', Uint8Array.from(grid.flags));
  bundle.add('floor', Float32Array.from(grid.floor));
  bundle.add('enclosure', Uint8Array.from(grid.enclosure));
  const cover = new Float32Array(ai.cover.points.length * 8);
  ai.cover.points.forEach((p, i) => {
    // Where to stand, which way the thing that covers is, and whether it hides a standing soldier.
    cover.set([p.x, p.y, p.z, p.dx, p.dz, p.high ? 1 : 0, p.dist ?? 0, 0], i * 8);
  });
  bundle.meta.coverSample = ai.cover.points.slice(0, 2).map((p) => JSON.parse(JSON.stringify(p, (k, value) => (value?.isVector3 ? value.toArray() : value))));
  bundle.add('cover', cover);
  await put('nav.dir', bundle.blob());
  log(`nav: ${grid.nx} × ${grid.nz} cells of ${grid.cell} m, ${grid.walkableCount} open, ${ai.cover.points.length} cover points`);
}

// ---------------------------------------------------------------- the sky and the lights
async function exportSky() {
  const sky = system('sky');
  const world = system('world');
  const SIDE = 512;
  const target = new THREE.WebGLRenderTarget(SIDE, SIDE, { type: THREE.FloatType, depthBuffer: false, minFilter: THREE.NearestFilter, magFilter: THREE.NearestFilter });
  const camera = new THREE.PerspectiveCamera(90, 1, 0.1, 2000);
  // The sky alone: the reference draws it as one triangle over the screen, from the camera's rays.
  const scene = new THREE.Scene();
  const dome = sky.dome.mesh;
  const parent = dome.parent;
  scene.add(dome);
  const bundle = new Bundle({ side: SIDE, faces: [] });
  const faces = [['px', [1, 0, 0], [0, 1, 0]], ['nx', [-1, 0, 0], [0, 1, 0]], ['py', [0, 1, 0], [0, 0, -1]], ['ny', [0, -1, 0], [0, 0, 1]], ['pz', [0, 0, 1], [0, 1, 0]], ['nz', [0, 0, -1], [0, 1, 0]]];
  const saved = renderer.getRenderTarget();
  for (const [name, look, up] of faces) {
    camera.position.set(0, 2, 0);
    camera.up.set(...up);
    camera.lookAt(look[0], 2 + look[1], look[2]);
    camera.updateMatrixWorld(true);
    renderer.setRenderTarget(target);
    renderer.clear();
    renderer.render(scene, camera);
    const raw = new Float32Array(SIDE * SIDE * 4);
    renderer.readRenderTargetPixels(target, 0, 0, SIDE, SIDE, raw);
    // Rows from the top, three floats a texel.
    const rgb = new Float32Array(SIDE * SIDE * 3);
    for (let y = 0; y < SIDE; y++) for (let x = 0; x < SIDE; x++) {
      const s = ((SIDE - 1 - y) * SIDE + x) * 4, d = (y * SIDE + x) * 3;
      rgb[d] = raw[s]; rgb[d + 1] = raw[s + 1]; rgb[d + 2] = raw[s + 2];
    }
    // The face's axes: a texel (x, y) looks along `forward + right·(2(x+0.5)/side − 1) − up·(2(y+0.5)/side − 1)`.
    const e = camera.matrixWorld.elements;
    bundle.meta.faces.push({ name, right: [e[0], e[1], e[2]], up: [e[4], e[5], e[6]], forward: [-e[8], -e[9], -e[10]] });
    bundle.add(name, rgb);
  }
  renderer.setRenderTarget(saved);
  parent?.add(dome);
  target.dispose();
  const sun = sky.sunLight;
  const direction = sun.position.clone().sub(sun.target.position).normalize();
  bundle.meta.sun = { direction: direction.toArray(), colour: sun.color.toArray(), intensity: sun.intensity };
  bundle.meta.hour = sky.hour;
  bundle.meta.ambient = sky.ambientColor?.toArray?.() ?? null;
  bundle.meta.fog = sky._fog ?? null;
  bundle.meta.lamps = [...(world.bulbs ?? []), ...(world.lamps ?? [])].map((l) => ({ position: l.getWorldPosition(new THREE.Vector3()).toArray(), colour: l.color.toArray(), intensity: l.intensity, distance: l.distance }));
  await put('sky.dir', bundle.blob());
  log(`sky: six faces of ${SIDE}, sun ${direction.toArray().map((c) => c.toFixed(3)).join(' ')} × ${sun.intensity.toFixed(2)}`);
}

// ---------------------------------------------------------------- from light to the display
// The reference shows a frame through its own exposure, tone curve and grade. The compiler bakes light in
// linear terms and needs the same last step, so the reference's composite pass is run here on a picture
// that holds every input of a table: `side`³ colours whose channels step evenly in log2 from `low` to
// `high` stops. Bloom, the lens' vignette, grain and sharpening are switched off: they belong to a frame,
// not to a surface.
async function exportTone() {
  const render = system('render');
  // Let the metering settle on the view a match starts with.
  await pump(40);
  const SIDE = 64, LOW = -9, HIGH = 5;
  const width = 512, height = (SIDE * SIDE * SIDE) / width;
  const input = new Float32Array(width * height * 4);
  const step = (i) => Math.pow(2, LOW + ((HIGH - LOW) * i) / (SIDE - 1));
  for (let b = 0; b < SIDE; b++) for (let g = 0; g < SIDE; g++) for (let r = 0; r < SIDE; r++) {
    const at = ((b * SIDE + g) * SIDE + r) * 4;
    input[at] = step(r); input[at + 1] = step(g); input[at + 2] = step(b); input[at + 3] = 1;
  }
  const source = new THREE.DataTexture(input, width, height, THREE.RGBAFormat, THREE.FloatType);
  source.minFilter = source.magFilter = THREE.NearestFilter;
  source.needsUpdate = true;
  const black = new THREE.DataTexture(new Float32Array([0, 0, 0, 1]), 1, 1, THREE.RGBAFormat, THREE.FloatType);
  black.needsUpdate = true;
  const u = render.composite.uniforms;
  const saved = { tColor: u.tColor.value, tBloom: u.tBloom.value, lens: u.uLens.value.clone(), grade: u.uGrade.value.clone(), texel: u.uTexel.value.clone() };
  u.tColor.value = source;
  u.tBloom.value = black;
  u.uLens.value.set(0, 0, 0, 0);
  u.uGrade.value.x = 0;
  u.uGrade.value.z = 0;
  u.uTexel.value.set(0, 0);
  const target = new THREE.WebGLRenderTarget(width, height, { depthBuffer: false, minFilter: THREE.NearestFilter, magFilter: THREE.NearestFilter });
  render.composite.render(renderer, target);
  const raw = new Uint8Array(width * height * 4);
  renderer.readRenderTargetPixels(target, 0, 0, width, height, raw);
  renderer.setRenderTarget(null);
  u.tColor.value = saved.tColor; u.tBloom.value = saved.tBloom; u.uLens.value.copy(saved.lens); u.uGrade.value.copy(saved.grade); u.uTexel.value.copy(saved.texel);
  target.dispose();
  const table = new Uint8Array(SIDE * SIDE * SIDE * 3);
  for (let i = 0; i < SIDE * SIDE * SIDE; i++) table.set(raw.subarray(i * 4, i * 4 + 3), i * 3);
  const exposure = render.debugExposure?.() ?? {};
  const bundle = new Bundle({ side: SIDE, low: LOW, high: HIGH, exposure });
  // Display code values, 0…255: red steps fastest, then green, then blue.
  bundle.add('table', table);
  await put('tone.dir', bundle.blob());
  log(`tone: ${SIDE}³ entries over ${HIGH - LOW} stops, exposure ${exposure.exposure?.toFixed?.(4)} at EV ${exposure.ev100?.toFixed?.(2)}`);
}

// ---------------------------------------------------------------- what the other files need to be read
async function exportScene() {
  const world = system('world');
  const physics = system('physics');
  const scene = {
    reference: 'Claude of Duty',
    // The map: the reference's own street, or a layout of ours built by the reference's kit.
    map: world.map ? { id: world.map.id, title: world.map.title ?? world.map.id } : { id: 'street', title: 'Market Street' },
    seed: 0x5eed1234,
    bounds: { min: world.bounds.min.toArray(), max: world.bounds.max.toArray() },
    // A place's floors lie at any height and have surfaces of its own: the cook's ground follows them.
    ...(world.ground ? { ground: world.ground } : {}),
    ...(world.light ? { light: world.light } : {}),
    ...(world.ladders ? { ladders: world.ladders } : {}),
    // The reference lays the street out along its own axes and turns it into the world: a level point
    // (x, z) stands at `origin + x·axisX + z·axisZ`.
    level: {
      origin: world.levelToWorld(0, 0, 0).toArray(),
      axisX: world.levelToWorld(1, 0, 0).sub(world.levelToWorld(0, 0, 0)).toArray(),
      axisZ: world.levelToWorld(0, 0, 1).sub(world.levelToWorld(0, 0, 0)).toArray(),
    },
    spawns: world.spawnPoints.map((s) => ({ position: s.position.toArray(), yaw: s.yaw, tag: s.tag })),
    buildings: world.buildings.map((b) => ({ id: b.spec.id, x: b.spec.x, z: b.spec.z, w: b.spec.w, d: b.spec.d, floors: b.spec.floors, roofY: b.roofY })),
    stats: world.stats,
    gravity: physics.gravity,
    // The three weapons: every number of the reference's definitions, and the view's climb and drift for
    // each shot of a burst as its seeded pattern gives them (pitch then yaw, radians).
    weapons: [...system('weapons').states.entries()].map(([id, state]) => ({ id, def: state.def, pattern: Array.from(state.pattern) })),
  };
  await put('scene.json', JSON.stringify(scene, null, 1));
  log('scene: written');
}

// A look at the map through the reference's own renderer: the camera at a level point, looking at
// another, for tools/export.ts to take a picture of. Nothing of it goes into a pack.
window.__LOOK__ = async ({ pos, look, fov = 60, frames = 12 }) => {
  const world = system('world');
  const player = system('player');
  player?.setControlEnabled?.(false);
  if (engine.viewScene) engine.viewScene.visible = false;
  const camera = engine.camera;
  // (a place's views are in its own coordinates: east, north, up)
  const at = (p) => (world.fromPlace ? world.fromPlace(p) : world.levelToWorld(p[0], p[1], p[2]));
  camera.position.copy(at(pos));
  camera.fov = fov;
  camera.updateProjectionMatrix();
  camera.lookAt(at(look));
  player?.teleport?.(camera.position, camera.rotation);
  await pump(frames);
  // (the player's own camera rig may have moved it: put it back for the frame that is looked at)
  camera.position.copy(at(pos));
  camera.lookAt(at(look));
  camera.updateMatrixWorld(true);
  await pump(2);
  return true;
};

// What the steps kept in their own files work with.
const ctx = { THREE, engine, pump, system, renderer, put, log, png, Bundle, readTexture, prepare, bakeVertices };
const steps = [['tone', exportTone], ['world', exportWorld], ['collision', exportCollision], ['nav', exportNav], ['sky', exportSky], ['figures', () => exportFigures(ctx)], ['audio', () => exportAudio(ctx)], ['scene', exportScene]];
for (const [name, step] of steps) {
  if (!wants(name)) continue;
  const t0 = performance.now();
  await step();
  log(`${name}: ${((performance.now() - t0) / 1000).toFixed(1)} s`);
}
await put('done', '');
log('done');
