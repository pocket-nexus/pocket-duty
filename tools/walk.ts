// Where the soldiers can walk on a map, read from its export before the eight minutes of a cook.
//
//   bun tools/duty.ts walk [--map yards] [--out f.png]
//
// The cook keeps the largest part of the walk grid that hangs together (crates/duty-cook/src/rules.rs: a
// step between two nodes is 45 cm at most) and closes the rest. This says the same from the export's
// `nav.dir`: the parts, the ground a soldier cannot reach, which starts stand on the part that stays, and
// a picture of it all from above (the world's axes: the level is turned in it).
//
// A map is fit for a match when every start stands on the part that stays and no ground a player can
// walk to is left out of it. It exits 1 when a start is off it.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { createCanvas } from "../vendor/pocketjs/node_modules/@napi-rs/canvas";
import { irDir, ROOT } from "./maps.ts";

/** The cook's rule (rules.rs): centimetres between two nodes a soldier steps. */
const STEP = 45;
/** A node this high over the common ground is a roof or an upper floor: no soldier is expected there. */
const ROOF = 2.5;
/** A part of this many nodes or more, off the part that stays, is an area and not the top of a thing. */
const AREA = 30;

interface Array_ {
  type: string;
  offset: number;
  length: number;
}

function open(path: string): { meta: any; array: (name: string) => Uint8Array | Float32Array } {
  const bytes = readFileSync(path);
  if (bytes.subarray(0, 4).toString() !== "DIR1") throw new Error(`${path} is not a DutyIR file`);
  const head = bytes.readUInt32LE(4);
  const meta = JSON.parse(bytes.subarray(8, 8 + head).toString("utf8"));
  const data = bytes.subarray(8 + head);
  return {
    meta,
    array(name) {
      const a = meta.arrays[name] as Array_ | undefined;
      if (!a) throw new Error(`${path} has no array ${name}`);
      const part = data.subarray(a.offset, a.offset + a.length * (a.type === "f32" ? 4 : 1));
      // (a copy: the file's bytes are not aligned for floats)
      return a.type === "f32" ? new Float32Array(new Uint8Array(part).buffer) : new Uint8Array(part);
    },
  };
}

export function walk(map: string, argv: string[]): void {
  const ir = irDir(map);
  const nav = open(join(ir, "nav.dir"));
  const { nx, nz, cell, minX, minZ } = nav.meta as { nx: number; nz: number; cell: number; minX: number; minZ: number };
  const flags = nav.array("flags") as Uint8Array, floor = nav.array("floor") as Float32Array;
  const count = nx * nz;
  const open_ = (i: number): boolean => flags[i] !== 0 && Number.isFinite(floor[i]!);
  const cm = (i: number): number => Math.round(floor[i]! * 100);

  const part = new Int32Array(count).fill(-1);
  const sizes: number[] = [];
  for (let start = 0; start < count; start++) {
    if (!open_(start) || part[start] !== -1) continue;
    const id = sizes.length;
    const stack = [start];
    part[start] = id;
    let size = 0;
    while (stack.length) {
      const node = stack.pop()!;
      size++;
      const i = node % nx, j = (node / nx) | 0;
      for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
        const ni = i + di, nj = j + dj;
        if (ni < 0 || nj < 0 || ni >= nx || nj >= nz) continue;
        const next = nj * nx + ni;
        // (a stair's nodes carry 4 beside their flag: a step to or from one may be twice as tall)
        if (open_(next) && part[next] === -1 && Math.abs(cm(node) - cm(next)) <= ((flags[node]! | flags[next]!) & 4 ? 2 * STEP : STEP)) {
          part[next] = id;
          stack.push(next);
        }
      }
    }
    sizes.push(size);
  }
  if (!sizes.length) throw new Error(`${relative(ROOT, ir)}/nav.dir has no open node`);
  const main = sizes.indexOf(Math.max(...sizes));
  const all = sizes.reduce((a, b) => a + b, 0);

  // The ground most of the part that stays lies at: what a node is measured against to be called raised, or a roof.
  const heights = new Map<number, number>();
  for (let i = 0; i < count; i++) if (part[i] === main) heights.set(Math.round(floor[i]! * 4), (heights.get(Math.round(floor[i]! * 4)) ?? 0) + 1);
  const low = [...heights].sort((a, b) => b[1] - a[1])[0]![0] / 4;
  // Off the part: a part of AREA nodes or more under a roof's height is ground no soldier reaches; a smaller one is the top of a thing.
  const lost = sizes.map((size, id) => ({ id, size, x: 0, z: 0, y: 0 })).filter((p) => p.id !== main && p.size >= AREA);
  let tops = 0;
  for (let i = 0; i < count; i++) {
    if (part[i] < 0 || part[i] === main || floor[i]! - low >= ROOF) continue;
    const area = lost.find((p) => p.id === part[i]);
    if (!area) tops++;
    else {
      area.x += (minX + (i % nx) * cell) / area.size;
      area.z += (minZ + ((i / nx) | 0) * cell) / area.size;
      area.y += floor[i]! / area.size;
    }
  }
  const ground = lost.filter((p) => p.y - low < ROOF && (p.x !== 0 || p.z !== 0));
  console.log(`walk: ${map}: ${nx} × ${nz} nodes of ${cell} m; ${sizes[main]} of ${all} open nodes hang together (${sizes.length} parts); ${tops} nodes are the tops of things`);
  for (const p of ground) console.log(`walk: ground off that part: ${p.size} nodes about world ${p.x.toFixed(1)}, ${p.z.toFixed(1)} at ${p.y.toFixed(2)} m`);
  if (!ground.length) console.log("walk: no ground is off that part");

  const spawns = (nav.meta.spawns ?? []) as { position: [number, number, number]; tag?: string }[];
  let off = 0;
  const marks: { x: number; z: number; on: boolean }[] = [];
  spawns.forEach((spawn, n) => {
    const [x, y, z] = spawn.position;
    const i = Math.floor((x - minX) / cell + 0.5), j = Math.floor((z - minZ) / cell + 0.5);
    const node = i >= 0 && j >= 0 && i < nx && j < nz ? j * nx + i : -1;
    const on = node >= 0 && part[node] === main && Math.abs(floor[node]! - y) < 0.6;
    if (!on) off++;
    marks.push({ x: (x - minX) / cell, z: (z - minZ) / cell, on });
    console.log(`walk: start ${n} ${(spawn.tag ?? "").padEnd(26)} ${on ? "on it" : node >= 0 && open_(node) ? `OFF it (a part of ${sizes[part[node]!]} nodes at ${floor[node]!.toFixed(2)} m)` : "OFF it (a closed node)"}`);
  });

  // The picture: the part that stays, the ground that is lost, roofs, starts.
  const S = 4;
  const canvas = createCanvas(nx * S, nz * S);
  const ctx = canvas.getContext("2d");
  ctx.fillStyle = "#14161a";
  ctx.fillRect(0, 0, nx * S, nz * S);
  for (let j = 0; j < nz; j++) {
    for (let i = 0; i < nx; i++) {
      const node = j * nx + i;
      if (part[node] < 0) continue;
      const height = floor[node]! - low;
      ctx.fillStyle = part[node] === main ? (height > 0.5 ? "#e8d9a0" : "#b9c2ae") : height >= ROOF ? "#3a4048" : sizes[part[node]!]! >= AREA ? "#e2483d" : "#7a6a55";
      ctx.fillRect(i * S, j * S, S, S);
    }
  }
  marks.forEach((mark, n) => {
    ctx.fillStyle = mark.on ? "#1f6feb" : "#ff00d4";
    ctx.beginPath();
    ctx.arc(mark.x * S + S / 2, mark.z * S + S / 2, 7, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#ffffff";
    ctx.font = "bold 11px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(n), mark.x * S + S / 2, mark.z * S + S / 2 + 0.5);
  });
  const at = argv.indexOf("--out");
  const out = at >= 0 && argv[at + 1] ? argv[at + 1]! : join(ROOT, ".pocket-build/validation/maps", map, "walk.png");
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, canvas.toBuffer("image/png"));
  console.log(`walk: ${relative(ROOT, out)} (light: the part that stays, sand: raised ground on it, red: ground off it, brown: tops of things, dark: roofs)`);
  if (off) {
    console.error(`walk: ${off} of ${spawns.length} starts are off the part that stays: move them, or open the way to them`);
    process.exit(1);
  }
}
