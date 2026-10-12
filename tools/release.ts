#!/usr/bin/env bun
// Pocket Duty's packages for Pocket Studio: one file per device, built from
// the checked-out commit by the commands a developer runs (tools/duty.ts,
// psp.ts, n3ds.ts).
//
//   bun tools/release.ts [--targets psp,3ds] [--out dist/release] [--no-build] [--no-cook] [--upload]
//
// For each target it cooks the profile's pack from the export
// (.pocket-build/duty/ir) with this commit's compiler, builds the program and
// writes one file to --out:
//
//   psp   pocket-duty-<version>-psp.zip   PSP/GAME/PocketDuty/, for the root of a Memory Stick
//   3ds   pocket-duty-<version>.3dsx      the pack and the interface in its ROMFS
//
// and release.json beside them: the commit, the version (ui/pocket.json),
// each file's size and SHA-256, the inputs and the toolchains. A target that
// fails is reported, the others still build, and the exit status is 1.
//
// --no-build      Take the packages release.json lists; their hashes are checked.
// --no-cook       Build from the pack already under .pocket-build/duty/<profile>
//                 (a cook takes four minutes); its receipt is recorded as it is.
// --upload        Send each package to Pocket Studio with `pocket-studio package`,
//                 run here, where `pocket-studio register` wrote .pocket-studio.json.
//                 POCKET_STUDIO_CLI names the command when it is not on PATH.
//
// No package goes to Git or to a GitHub release.

import { closeSync, cpSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync, writeSync } from "node:fs";
import { join, resolve } from "node:path";
import { deflateRawSync } from "node:zlib";

const ROOT = resolve(import.meta.dir, "..");
const POCKETJS = join(ROOT, "vendor/pocketjs");
const WORK = join(ROOT, ".pocket-build/release");
const NAME = "pocket-duty";
const TITLE = "Pocket Duty";

const argv = process.argv.slice(2);
const option = (key: string, fallback: string) => {
  const at = argv.indexOf(key);
  return at < 0 ? fallback : (argv[at + 1] ?? fallback);
};
const OUT = resolve(option("--out", join(ROOT, "dist/release")));

/** A package as release.json lists it. */
interface Package {
  target: Target;
  filename: string;
  bytes: number;
  sha256: string;
}

/** Pocket Studio's ids for the devices this repository builds for. */
const TARGETS = ["psp", "3ds"] as const;
type Target = (typeof TARGETS)[number];

/** What each target cooks and what its package is called. */
const BUILDS: Record<Target, { profile: string; filename: (version: string) => string; build: (log: string, output: string) => Promise<void> }> = {
  psp: { profile: "psp30", filename: (v) => `${NAME}-${v}-psp.zip`, build: psp },
  "3ds": { profile: "n3ds30", filename: (v) => `${NAME}-${v}.3dsx`, build: n3ds },
};

const sha256 = (bytes: Uint8Array | string) => new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
const fileSha256 = (path: string) => sha256(readFileSync(path));

/** The first line a command prints, or null when it cannot run. */
function line(command: string[], cwd = ROOT): string | null {
  try {
    const done = Bun.spawnSync(command, { cwd, stdout: "pipe", stderr: "pipe" });
    return done.exitCode === 0 ? (done.stdout.toString().trim().split("\n")[0] ?? "") : null;
  } catch {
    return null;
  }
}

/** Runs a build command with its output in `log`; a failure carries the log's last lines. */
async function run(log: string, command: string[], env: Record<string, string> = {}): Promise<void> {
  const fd = openSync(log, "a");
  writeSync(fd, `\n$ ${command.join(" ")}\n`);
  const code = await Bun.spawn(command, { cwd: ROOT, stdin: "ignore", stdout: fd, stderr: fd, env: { ...process.env, ...env } }).exited;
  closeSync(fd);
  if (code !== 0) throw new Error(`\`${command.join(" ")}\` exited ${code}; ${log} ends:\n${readFileSync(log, "utf8").trimEnd().split("\n").slice(-20).join("\n")}`);
}

// ---------------------------------------------------------------- archives

/** Every file and directory under `directory`, named from `prefix`. A directory's name ends in a slash and it has no path. */
function tree(directory: string, prefix: string): { name: string; path?: string }[] {
  return [
    { name: `${prefix}/` },
    ...readdirSync(directory, { withFileTypes: true }).flatMap((entry) =>
      entry.isDirectory() ? tree(join(directory, entry.name), `${prefix}/${entry.name}`) : [{ name: `${prefix}/${entry.name}`, path: join(directory, entry.name) }],
    ),
  ];
}

/**
 * Writes a zip whose bytes follow from its entries alone: the entries named
 * in `first`, then the others in the order of their names, every date
 * 1980-01-01 00:00, modes 0644, or 0755 for a directory and for a file its
 * owner may execute, no extra fields. An entry is deflated at level 6 unless
 * that makes it longer.
 */
function writeZip(output: string, entries: { name: string; path?: string }[], first: string[] = []): void {
  const rank = (name: string) => (first.includes(name) ? first.indexOf(name) : first.length);
  const fd = openSync(output, "w");
  const directory: Buffer[] = [];
  let at = 0;
  for (const entry of [...entries].sort((a, b) => rank(a.name) - rank(b.name) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
    const data = entry.path === undefined ? Buffer.alloc(0) : readFileSync(entry.path);
    const deflated = data.length ? deflateRawSync(data, { level: 6 }) : data;
    const body = deflated.length < data.length ? deflated : data;
    const method = body === data ? 0 : 8;
    const mode = entry.path === undefined ? 0o040755 : statSync(entry.path).mode & 0o100 ? 0o100755 : 0o100644;
    const name = Buffer.from(entry.name);
    const crc = Bun.hash.crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(method, 8);
    local.writeUInt16LE(0x0021, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(name.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    // Made on Unix, so the upper half of the external attributes is a file mode.
    central.writeUInt16LE((3 << 8) | 20, 4);
    local.copy(central, 6, 4, 30);
    central.writeUInt32LE(((mode << 16) | (entry.path === undefined ? 0x10 : 0)) >>> 0, 38);
    central.writeUInt32LE(at, 42);
    directory.push(central, name);
    for (const part of [local, name, body]) writeSync(fd, part);
    at += local.length + name.length + body.length;
  }
  const list = Buffer.concat(directory);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(directory.length / 2, 8);
  end.writeUInt16LE(directory.length / 2, 10);
  end.writeUInt32LE(list.length, 12);
  end.writeUInt32LE(at, 16);
  writeSync(fd, list);
  writeSync(fd, end);
  closeSync(fd);
}

// ---------------------------------------------------------------- targets

/** The Memory Stick folder (`tools/psp.ts` `package`), zipped from the card's root. */
async function psp(log: string, output: string): Promise<void> {
  rmSync(join(ROOT, "dist/psp/PSP"), { recursive: true, force: true });
  await run(log, ["bun", "tools/psp.ts", "build"]);
  await run(log, ["bun", "tools/psp.ts", "package"]);
  writeZip(output, [{ name: "PSP/" }, { name: "PSP/GAME/" }, ...tree(join(ROOT, "dist/psp/PSP/GAME/PocketDuty"), "PSP/GAME/PocketDuty")]);
}

/** The `.3dsx` with the pack and the interface in its ROMFS (`tools/n3ds.ts` `build`). */
async function n3ds(log: string, output: string): Promise<void> {
  await run(log, ["bun", "tools/n3ds.ts", "build"]);
  cpSync(join(ROOT, "dist/3ds/pocket-duty.3dsx"), output);
}

/** A package is refused when it is empty or holds what only a development run uses. */
function accept(output: string): void {
  if (statSync(output).size < 1024 * 1024) throw new Error(`${output} is ${statSync(output).size} bytes: the pack is not in it`);
}

/** The export every pack is cooked from: its files' names and bytes as one hash. */
function dutyIr() {
  const directory = join(ROOT, ".pocket-build/duty/ir");
  if (!existsSync(directory)) throw new Error(`${directory} is missing: run \`bun tools/duty.ts export\` first`);
  const hash = new Bun.CryptoHasher("sha256");
  let files = 0;
  const walk = (at: string, prefix: string) => {
    for (const entry of readdirSync(at, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      if (entry.isDirectory()) walk(join(at, entry.name), `${prefix}${entry.name}/`);
      else {
        hash.update(`${prefix}${entry.name}`).update(readFileSync(join(at, entry.name)));
        files++;
      }
    }
  };
  walk(directory, "");
  return { files, sha256: hash.digest("hex"), reference: line(["git", "log", "-1", "--format=%H", "--", "web/src", ":!web/src/pocket"]) };
}

function toolchains() {
  const rustc = (toolchain: string | null) => (toolchain ? `${toolchain}: ${line(["rustup", "run", toolchain, "rustc", "-V"]) ?? "not installed"}` : null);
  const pinned = (file: string) => JSON.parse(readFileSync(join(POCKETJS, "tools/cli", file), "utf8"));
  const container = /"(devkitpro\/devkitarm@sha256:[0-9a-f]{64})"/.exec(readFileSync(join(POCKETJS, "tools/3ds-toolchain.ts"), "utf8"))?.[1] ?? null;
  const n3dsTool = join(ROOT, "tools/n3ds.ts");
  return {
    pocketjs: line(["git", "-C", POCKETJS, "rev-parse", "HEAD"]),
    bun: Bun.version,
    rustc: {
      cook: line(["rustc", "-V"]),
      psp: rustc(pinned("psp-toolchain.json").rust.toolchain),
      "3ds": existsSync(n3dsTool) ? rustc(/TOOLCHAIN = "([^"]+)"/.exec(readFileSync(n3dsTool, "utf8"))?.[1] ?? null) : null,
    },
    pspSdk: pinned("psp-toolchain.json").sdk.sha256 as string,
    devkitarm: container,
  };
}

// ---------------------------------------------------------------- Pocket Studio

/** Sends the packages with the Studio's own CLI, from this directory's project link. */
async function upload(packages: Package[], version: string): Promise<boolean> {
  const link = join(ROOT, ".pocket-studio.json");
  if (!existsSync(link)) {
    console.error(
      `release: ${link} is missing, so this checkout names no Pocket Studio project. Once, in ${ROOT}:\n` +
        `  pocket-studio link <CODE>                       # the link code from the room, when this computer is not linked to an account\n` +
        `  pocket-studio register --title "${TITLE}"   # writes .pocket-studio.json, which Git ignores\n` +
        `then: bun tools/release.ts --no-build --upload`,
    );
    return false;
  }
  const project = JSON.parse(readFileSync(link, "utf8"));
  if (project.kind !== "site") throw new Error(`${link} links a device session, not a registered game: run \`pocket-studio register --title "${TITLE}"\` in a directory without one`);
  const cli = process.env.POCKET_STUDIO_CLI?.trim().split(/\s+/) ?? (Bun.which("pocket-studio") ? ["pocket-studio"] : null);
  if (!cli) throw new Error("no pocket-studio on PATH: install it from the Studio, or set POCKET_STUDIO_CLI to its command");
  console.log(`release: uploading ${packages.length} package(s) to ${project.server} (${project.app})`);
  for (const pkg of packages) {
    const command = [...cli, "package", join(OUT, pkg.filename), "--target", pkg.target, "--version", version];
    console.log(`$ ${command.join(" ")}`);
    const code = await Bun.spawn(command, { cwd: ROOT, stdin: "ignore", stdout: "inherit", stderr: "inherit" }).exited;
    if (code !== 0) throw new Error(`pocket-studio package exited ${code} for ${pkg.filename}`);
  }
  return true;
}

// ---------------------------------------------------------------- main

const asked = option("--targets", TARGETS.join(",")).split(",").filter(Boolean);
const unknown = asked.filter((t) => !(TARGETS as readonly string[]).includes(t));
if (unknown.length || argv.includes("--help")) {
  if (unknown.length) console.error(`release: no build for ${unknown.join(", ")}: the targets are ${TARGETS.join(", ")}`);
  console.log("usage: bun tools/release.ts [--targets psp,3ds] [--out dist/release] [--no-build] [--no-cook] [--upload]");
  process.exit(unknown.length ? 1 : 0);
}
const targets = TARGETS.filter((t) => asked.includes(t));
const version = JSON.parse(readFileSync(join(ROOT, "ui/pocket.json"), "utf8")).version as string;
const commit = line(["git", "rev-parse", "HEAD"]);
const dirty = Bun.spawnSync(["git", "status", "--porcelain"], { cwd: ROOT }).stdout.toString().trim() !== "";
const record = join(OUT, "release.json");
if (argv.includes("--upload") && dirty) throw new Error("the checkout has uncommitted changes: a package names the commit it was built from, so commit before --upload");
let packages: Package[] = [];
const failed: { target: Target; error: string }[] = [];

if (argv.includes("--no-build")) {
  if (!existsSync(record)) throw new Error(`${record} is missing: run without --no-build first`);
  const built = JSON.parse(readFileSync(record, "utf8"));
  if (built.commit !== commit) throw new Error(`${record} was built from ${built.commit}; the checkout is at ${commit}`);
  packages = (built.packages as Package[]).filter((p) => targets.includes(p.target));
  for (const pkg of packages) {
    if (fileSha256(join(OUT, pkg.filename)) !== pkg.sha256) throw new Error(`${pkg.filename} is not the file release.json lists`);
    console.log(`release: ${pkg.target.padEnd(4)} ${pkg.filename}  ${pkg.bytes} bytes  sha256 ${pkg.sha256}`);
  }
} else {
  mkdirSync(OUT, { recursive: true });
  mkdirSync(join(WORK, "logs"), { recursive: true });
  console.log(`release: ${TITLE} ${version} at ${commit}${dirty ? " with uncommitted changes" : ""}`);
  const ir = dutyIr();
  const packs: Record<string, { bytes: number; sha256: string }> = {};
  const seconds: Partial<Record<Target, number>> = {};
  // One at a time: each device's interface is compiled into the same generated files under ui/.
  for (const target of targets) {
    const { profile, filename, build } = BUILDS[target];
    const log = join(WORK, "logs", `${target}.log`);
    const output = join(OUT, filename(version));
    const start = performance.now();
    rmSync(log, { force: true });
    rmSync(output, { force: true });
    try {
      console.log(`release: ${target}: ${argv.includes("--no-cook") ? "taking the cooked" : "cooking"} ${profile}, building (log: ${log})`);
      if (!argv.includes("--no-cook")) await run(log, ["bun", "tools/duty.ts", "cook", "--profile", profile]);
      packs[profile] = JSON.parse(readFileSync(join(ROOT, ".pocket-build/duty", profile, "receipt.json"), "utf8")).pack;
      await build(log, output);
      accept(output);
      packages.push({ target, filename: filename(version), bytes: statSync(output).size, sha256: fileSha256(output) });
    } catch (error) {
      rmSync(output, { force: true });
      failed.push({ target, error: error instanceof Error ? error.message : String(error) });
      console.error(`release: ${target} failed: ${failed.at(-1)!.error}`);
    }
    seconds[target] = Math.round((performance.now() - start) / 100) / 10;
  }
  writeFileSync(
    record,
    JSON.stringify(
      {
        schema: 1,
        name: NAME,
        title: TITLE,
        version,
        commit,
        dirty,
        packages,
        failed: failed.map(({ target, error }) => ({ target, error: error.split("\n")[0] })),
        inputs: { dutyIr: ir, packs },
        toolchains: toolchains(),
      },
      null,
      2,
    ) + "\n",
  );
  for (const pkg of packages) console.log(`release: ${pkg.target.padEnd(4)} ${pkg.filename}  ${pkg.bytes} bytes  sha256 ${pkg.sha256}  ${seconds[pkg.target]} s`);
  for (const { target } of failed) console.log(`release: ${target.padEnd(4)} not built  ${seconds[target]} s`);
  console.log(`release: ${record}`);
}

if (argv.includes("--upload")) {
  if (failed.length) throw new Error(`not uploading: ${failed.map((f) => f.target).join(", ")} did not build`);
  if (!(await upload(packages, version))) process.exit(1);
}
process.exit(failed.length ? 1 : 0);
