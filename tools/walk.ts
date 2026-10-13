// Where the soldiers can walk on a map, read from its export before the eight minutes of a cook.
//
//   bun tools/duty.ts walk [--map yards] [--out f.png]
//
// The cook keeps the largest part of the walk grid that hangs together (crates/duty-cook/src/rules.rs: a
// step between two nodes is 45 cm at most) and closes the rest. This says the same from the export's
// `nav.dir`: the parts, the ground a soldier cannot reach, which starts stand on the part that stays, and
// a picture of it all from above (the world's axes: the level is turned in it).
//
// Where a floor stands over a floor the export has a further node for the cell (`overCell`, `overFlags`,
// `overFloor`), and the scene lists the ladders: a step goes to whichever node of the next cell is within
// its height, and a ladder joins the nodes at its foot and its top, as in the cook.
//
// A map is fit for a match when every start stands on the part that stays and no ground a player can
// walk to is left out of it. It exits 1 when a start is off it.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
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

function open(path: string): { meta: any; array: (name: string) => Uint8Array | Float32Array | Uint32Array } {
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
      const wide = a.type === "f32" || a.type === "u32";
      const part = data.subarray(a.offset, a.offset + a.length * (wide ? 4 : 1));
      // (a copy: the file's bytes are not aligned for floats)
      return a.type === "f32" ? new Float32Array(new Uint8Array(part).buffer) : a.type === "u32" ? new Uint32Array(new Uint8Array(part).buffer) : new Uint8Array(part);
    },
  };
}

export function walk(map: string, argv: string[]): void {
  const ir = irDir(map);
  const nav = open(join(ir, "nav.dir"));
  const { nx, nz, cell, minX, minZ } = nav.meta as { nx: number; nz: number; cell: number; minX: number; minZ: number };
  const cells = nx * nz;
  // The grid's nodes, then the further ones: a flag, a floor and a cell each.
  const further = nav.meta.arrays.overCell ? (nav.array("overCell") as Uint32Array) : new Uint32Array(0);
  const count = cells + further.length;
  const flags = new Uint8Array(count), floor = new Float32Array(count);
  flags.set(nav.array("flags") as Uint8Array);
  floor.set(nav.array("floor") as Float32Array);
  if (further.length) {
    flags.set(nav.array("overFlags") as Uint8Array, cells);
    floor.set(nav.array("overFloor") as Float32Array, cells);
  }
  const cellOf = (node: number): number => (node < cells ? node : further[node - cells]!);
  const over = new Map<number, number[]>();
  further.forEach((cell, k) => over.set(cell, [...(over.get(cell) ?? []), cells + k]));
  const at = (cell: number): number[] => [cell, ...(over.get(cell) ?? [])];
  const open_ = (i: number): boolean => flags[i] !== 0 && Number.isFinite(floor[i]!);
  const cm = (i: number): number => Math.round(floor[i]! * 100);

  // The scene's ladders: the open nodes at a ladder's foot and at its top, within a cell of each.
  const scenePath = join(ir, "scene.json");
  const scene = existsSync(scenePath) ? (JSON.parse(readFileSync(scenePath, "utf8")) as { ladders?: { foot: number[]; top: number[] }[] }) : {};
  const near = ([x, y, z]: number[]): number => {
    const ci = Math.floor((x! - minX) / cell + 0.5), cj = Math.floor((z! - minZ) / cell + 0.5);
    let best = -1, least = Infinity;
    for (let dj = -1; dj <= 1; dj++) {
      for (let di = -1; di <= 1; di++) {
        const i = ci + di, j = cj + dj;
        if (i < 0 || j < 0 || i >= nx || j >= nz) continue;
        for (const node of at(j * nx + i)) if (open_(node) && Math.abs(floor[node]! - y!) < 0.6 && di * di + dj * dj < least) [best, least] = [node, di * di + dj * dj];
      }
    }
    return best;
  };
  const ladders = (scene.ladders ?? []).map((l) => ({ ...l, ends: [near(l.foot), near(l.top)] as [number, number] }));

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
      const i = cellOf(node) % nx, j = (cellOf(node) / nx) | 0;
      for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
        const ni = i + di, nj = j + dj;
        if (ni < 0 || nj < 0 || ni >= nx || nj >= nz) continue;
        for (const next of at(nj * nx + ni)) {
          // (a stair's nodes carry 4 beside their flag: a step to or from one may be twice as tall)
          if (open_(next) && part[next] === -1 && Math.abs(cm(node) - cm(next)) <= ((flags[node]! | flags[next]!) & 4 ? 2 * STEP : STEP)) {
            part[next] = id;
            stack.push(next);
          }
        }
      }
      for (const { ends } of ladders) {
        const next = ends[0] === node ? ends[1] : ends[1] === node ? ends[0] : -1;
        if (next >= 0 && ends[0] >= 0 && ends[1] >= 0 && part[next] === -1) {
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
      area.x += (minX + (cellOf(i) % nx) * cell) / area.size;
      area.z += (minZ + ((cellOf(i) / nx) | 0) * cell) / area.size;
      area.y += floor[i]! / area.size;
    }
  }
  const ground = lost.filter((p) => p.y - low < ROOF && (p.x !== 0 || p.z !== 0));
  console.log(`walk: ${map}: ${nx} × ${nz} nodes of ${cell} m; ${sizes[main]} of ${all} open nodes hang together (${sizes.length} parts); ${tops} nodes are the tops of things`);
  for (const p of ground) console.log(`walk: ground off that part: ${p.size} nodes about world ${p.x.toFixed(1)}, ${p.z.toFixed(1)} at ${p.y.toFixed(2)} m`);
  if (!ground.length) console.log("walk: no ground is off that part");
  if (further.length || ladders.length) {
    let kept = 0;
    for (let node = cells; node < count; node++) if (part[node] === main) kept++;
    console.log(`walk: ${kept} of ${further.length} nodes on floors over floors are on that part; ${ladders.filter((l) => l.ends.every((node) => node >= 0 && part[node] === main)).length} of ${ladders.length} ladders are a soldier's way`);
  }

  const spawns = (nav.meta.spawns ?? []) as { position: [number, number, number]; tag?: string }[];
  let off = 0;
  const marks: { x: number; z: number; on: boolean }[] = [];
  spawns.forEach((spawn, n) => {
    const [x, y, z] = spawn.position;
    const i = Math.floor((x - minX) / cell + 0.5), j = Math.floor((z - minZ) / cell + 0.5);
    // (of the cell's nodes, the one at the start's height)
    const here = i >= 0 && j >= 0 && i < nx && j < nz ? at(j * nx + i) : [];
    const node = here.find((each) => open_(each) && Math.abs(floor[each]! - y) < 0.6) ?? here[0] ?? -1;
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
  // A node on a floor over a floor: a blue middle in its cell where it is on the part that stays, red or brown where it is not.
  for (let node = cells; node < count; node++) {
    if (part[node]! < 0) continue;
    ctx.fillStyle = part[node] === main ? "#3f7fd9" : sizes[part[node]!]! >= AREA ? "#e2483d" : "#7a6a55";
    ctx.fillRect((cellOf(node) % nx) * S + 1, ((cellOf(node) / nx) | 0) * S + 1, S - 2, S - 2);
  }
  // A ladder: green where a soldier climbs it, grey where only a player does.
  for (const { foot, top, ends } of ladders) {
    ctx.strokeStyle = ends.every((node) => node >= 0 && part[node] === main) ? "#35d07f" : "#9aa0a6";
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(((foot[0]! - minX) / cell) * S + S / 2, ((foot[2]! - minZ) / cell) * S + S / 2);
    ctx.lineTo(((top[0]! - minX) / cell) * S + S / 2, ((top[2]! - minZ) / cell) * S + S / 2);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(((foot[0]! - minX) / cell) * S + S / 2, ((foot[2]! - minZ) / cell) * S + S / 2, 4, 0, Math.PI * 2);
    ctx.stroke();
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
  const flag = argv.indexOf("--out");
  const out = flag >= 0 && argv[flag + 1] ? argv[flag + 1]! : join(ROOT, ".pocket-build/validation/maps", map, "walk.png");
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, canvas.toBuffer("image/png"));
  console.log(`walk: ${relative(ROOT, out)} (light: the part that stays, sand: raised ground on it, red: ground off it, brown: tops of things, dark: roofs${further.length || ladders.length ? ", blue: a floor over a floor, green: a ladder" : ""})`);
  if (off) {
    console.error(`walk: ${off} of ${spawns.length} starts are off the part that stays: move them, or open the way to them`);
    process.exit(1);
  }
}
