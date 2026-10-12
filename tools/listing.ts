#!/usr/bin/env bun
// The game's listing on Pocket Studio: the words of listing/listing.json and
// the pictures it names, filmed from the game itself.
//
//   bun tools/listing.ts [--only NAME…]   → dist/listing/: listing.json, the clips and their posters,
//                                           the stills, the share picture; and dist/cover.png
//   bun tools/listing.ts --words          listing.json alone, for the pictures already in dist/listing/
//   bun tools/listing.ts --emu            the same pictures drawn by PPSSPPHeadless: for trying a take
//   bun tools/listing.ts --upload         then `pocket-studio listing dist/listing`, run here, where
//                                         `pocket-studio register` wrote .pocket-studio.json.
//                                         POCKET_STUDIO_CLI names the command when it is not on PATH.
//
// Every picture is drawn by a PSP: `from: "psp"` in the listing. A run is the program started with a
// `boot.txt` of control words (`runs` in listing/listing.json; the words are `remote` in psp/src/main.rs),
// filmed from its first frame by `bun tools/psp.ts film`: the console steps the match two sixtieths of a
// second a frame and writes each frame's picture to the PSPLINK share, so a run of the same words on the
// same build gives the same pictures. A take says what of a run is kept:
//
//   { "run", "from", "frames", "poster" }   a clip: frames `from` to `from + frames` at 30 a second;
//                                           the poster is frame `from + poster`
//   { "run", "frame" }                      a still: frame `frame`
//
// A run with `bare` has no interface over the street: the share picture and the cover come from one.
// A clip and a still are the console's 480 × 272 with each texel doubled (960 × 544). A clip fades from
// black and to black over a third of a second, so it loops through a dip.
//
// A filmed run is kept in .pocket-build/listing/runs/ with the build it was filmed from, and filmed again
// when the words or the build change. `--upload` refuses pictures the emulator drew: the listing says a
// PSP drew them. No picture goes to Git: dist/ and .pocket-build/ are ignored.
//
// It needs the built program (`bun tools/psp.ts build`), a PSP on the cable under PSPLINK, ffmpeg and ffprobe.

import { $ } from "bun";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const OUT = join(ROOT, "dist/listing");
const COVER = join(ROOT, "dist/cover.png");
const WORK = join(ROOT, ".pocket-build/listing");
const BUILT = join(ROOT, "dist/psp");

/** The PSP's screen, a frame's bytes (16 bits a texel), a clip's frames a second, and the frames a clip fades over. */
const WIDTH = 480, HEIGHT = 272, FRAME = WIDTH * HEIGHT * 2, RATE = 30, FADE = 10;
/** A picture in the listing: each texel doubled. */
const SHOWN = { width: WIDTH * 2, height: HEIGHT * 2 };
const MIB = 1 << 20;
/** What Pocket Studio takes (the listing's contract). */
const LIMIT = { clip: 12 * MIB, still: 2 * MIB, card: 1 * MIB, total: 96 * MIB, seconds: [6, 30], media: [2, 12], tagline: 120, paragraph: 600, paragraphs: [1, 6], caption: 140 };
const CARD = { width: 1200, height: 630 };

interface Run { words: string; bare?: boolean }
interface Take { run: string; from?: number; frames?: number; poster?: number; frame?: number }
interface Media { kind: "video" | "image"; file: string; poster?: string; take: Take; caption: string }
interface Source {
  tagline: string;
  description: string[];
  runs: Record<string, Run>;
  media: Media[];
  card: string;
  cardTake: Take;
  coverTake: Take;
  translations?: Record<string, { tagline?: string; description?: string[]; captions?: Record<string, string> }>;
}

const argv = process.argv.slice(2);
const flag = (name: string) => argv.includes(name);
const only = argv.includes("--only") ? argv.slice(argv.indexOf("--only") + 1).filter((a) => !a.startsWith("--")) : null;
const wanted = (file: string) => !only || only.includes(file);
const source: Source = JSON.parse(readFileSync(join(ROOT, "listing/listing.json"), "utf8"));

const sha = (path: string): string => new Bun.CryptoHasher("sha256").update(readFileSync(path)).digest("hex");

/** What a run was filmed from: the program, the interface's files and the pack. */
function build(): string {
  const files = ["pocket-duty.prx", "duty.js", "duty.pak"].map((f) => join(BUILT, f));
  for (const f of files) if (!existsSync(f)) throw new Error(`no ${f}: bun tools/psp.ts build`);
  const pack = JSON.parse(readFileSync(join(ROOT, ".pocket-build/duty/psp30/receipt.json"), "utf8")).pack.sha256;
  return new Bun.CryptoHasher("sha256").update([...files.map(sha), pack].join("\n")).digest("hex");
}

/** The frames a run must have for the takes that are wanted. */
function needed(name: string): number {
  let frames = 0;
  const count = (take: Take) => {
    if (take.run === name) frames = Math.max(frames, take.frame !== undefined ? take.frame + 1 : (take.from ?? 0) + (take.frames ?? 0));
  };
  for (const media of source.media) if (wanted(media.file) || (media.poster && wanted(media.poster))) count(media.take);
  if (wanted(source.card)) count(source.cardTake);
  if (wanted("cover.png")) count(source.coverTake);
  return frames;
}

/** The directory of a run's frames, filmed now unless it holds them already. Returns who drew them. */
async function film(name: string, frames: number): Promise<{ dir: string; drawnBy: string }> {
  const run = source.runs[name];
  if (!run) throw new Error(`listing.json names the run "${name}" and has none of that name`);
  const dir = join(WORK, "runs", flag("--emu") ? `${name}-emu` : name);
  const note = join(dir, "run.json");
  const identity = { words: run.words, bare: !!run.bare, build: build() };
  if (existsSync(note)) {
    const kept = JSON.parse(readFileSync(note, "utf8"));
    if (kept.words === identity.words && kept.bare === identity.bare && kept.build === identity.build && kept.frames >= frames) return { dir, drawnBy: kept.drawnBy };
  }
  console.log(`listing: filming "${name}", ${frames} frames${flag("--emu") ? " in PPSSPPHeadless" : " on the PSP"}`);
  const said = await $`bun tools/psp.ts film --boot ${run.words} --frames ${frames} --from 1 --out ${dir} ${run.bare ? ["--bare"] : []} ${flag("--emu") ? ["--emu"] : []}`.cwd(ROOT).text();
  const result = JSON.parse(said.trim().split("\n").at(-1)!);
  writeFileSync(note, JSON.stringify({ ...identity, frames, drawnBy: result.drawnBy, filmedAt: new Date().toISOString(), match: result.match }, null, 1));
  return { dir, drawnBy: result.drawnBy };
}

const frameFile = (dir: string, index: number) => join(dir, `${String(index).padStart(5, "0")}.raw`);

/** ffmpeg reading `count` frames of a run from `first`, with `encode` after its input. */
async function encode(dir: string, first: number, count: number, encode: string[]): Promise<void> {
  const coder = Bun.spawn(["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "bgr565le", "-s", `${WIDTH}x${HEIGHT}`, "-r", String(RATE), "-i", "-", ...encode], { stdin: "pipe", stdout: "inherit", stderr: "inherit" });
  for (let i = first; i < first + count; i++) {
    const bytes = readFileSync(frameFile(dir, i));
    if (bytes.length !== FRAME) throw new Error(`${frameFile(dir, i)} is ${bytes.length} bytes, not a frame's ${FRAME}`);
    coder.stdin.write(bytes);
  }
  coder.stdin.end();
  if ((await coder.exited) !== 0) throw new Error("ffmpeg failed");
}

/** A JPEG of one frame. `-q:v 2` is ffmpeg's second finest quantiser; the chroma is not halved. */
const jpeg = (filter: string) => ["-frames:v", "1", "-vf", `${filter},scale=in_range=pc:out_range=pc:out_color_matrix=bt601,format=yuvj444p`, "-q:v", "2", "-map_metadata", "-1", "-fflags", "+bitexact"];
const DOUBLED = `scale=${SHOWN.width}:${SHOWN.height}:flags=neighbor`;

async function pictures(): Promise<Record<string, string>> {
  mkdirSync(OUT, { recursive: true });
  const drawn: Record<string, string> = {};
  const runs = new Map<string, { dir: string; drawnBy: string }>();
  for (const name of Object.keys(source.runs)) {
    const frames = needed(name);
    if (frames > 0) runs.set(name, await film(name, frames));
  }
  const of = (take: Take) => runs.get(take.run)!;
  for (const media of source.media) {
    const { take } = media;
    if (media.kind === "video") {
      const [from, frames] = [take.from!, take.frames!];
      if (wanted(media.file)) {
        const seconds = frames / RATE;
        const fade = (FADE / RATE).toFixed(4);
        await encode(of(take).dir, from, frames, [
          "-vf", `${DOUBLED},fade=t=in:st=0:d=${fade},fade=t=out:st=${(seconds - FADE / RATE).toFixed(4)}:d=${fade},format=yuv420p`,
          "-c:v", "libx264", "-preset", "slow", "-crf", "17", "-profile:v", "high", "-g", String(RATE * 2), "-movflags", "+faststart", "-an", "-map_metadata", "-1", "-fflags", "+bitexact",
          join(OUT, media.file),
        ]);
        drawn[media.file] = of(take).drawnBy;
      }
      if (media.poster && wanted(media.poster)) {
        await encode(of(take).dir, from + (take.poster ?? 0), 1, [...jpeg(DOUBLED), join(OUT, media.poster)]);
        drawn[media.poster] = of(take).drawnBy;
      }
    } else if (wanted(media.file)) {
      await encode(of(take).dir, take.frame!, 1, [...jpeg(DOUBLED), join(OUT, media.file)]);
      drawn[media.file] = of(take).drawnBy;
    }
  }
  if (wanted(source.card)) {
    // The share picture: the frame at two and a half times its size, each texel an even block, cut to the card's height.
    const tall = Math.round((HEIGHT * CARD.width) / WIDTH);
    await encode(of(source.cardTake).dir, source.cardTake.frame!, 1, [...jpeg(`scale=${WIDTH * 5}:${HEIGHT * 5}:flags=neighbor,scale=${CARD.width}:${tall}:flags=area,crop=${CARD.width}:${CARD.height}:0:${Math.round((tall - CARD.height) / 2)}`), join(OUT, source.card)]);
    drawn[source.card] = of(source.cardTake).drawnBy;
  }
  if (wanted("cover.png")) {
    // The cover: a frame at the console's own size, which is what a game's case shows.
    await encode(of(source.coverTake).dir, source.coverTake.frame!, 1, ["-frames:v", "1", "-map_metadata", "-1", "-fflags", "+bitexact", COVER]);
    drawn["cover.png"] = of(source.coverTake).drawnBy;
  }
  const note = join(WORK, "drawn.json");
  const before = existsSync(note) ? JSON.parse(readFileSync(note, "utf8")) : {};
  writeFileSync(note, JSON.stringify({ ...before, ...drawn }, null, 1));
  return { ...before, ...drawn };
}

/** dist/listing/listing.json: the words, and each picture's size and length as its file has them. */
async function words(): Promise<void> {
  const fail = (text: string): never => {
    throw new Error(`listing: ${text}`);
  };
  if (source.tagline.length > LIMIT.tagline) fail(`the tagline is ${source.tagline.length} characters; at most ${LIMIT.tagline}`);
  if (source.description.length < LIMIT.paragraphs[0]! || source.description.length > LIMIT.paragraphs[1]!) fail(`the description has ${source.description.length} paragraphs`);
  for (const paragraph of source.description) if (paragraph.length > LIMIT.paragraph) fail(`a paragraph is ${paragraph.length} characters; at most ${LIMIT.paragraph}`);
  if (source.media.length < LIMIT.media[0]! || source.media.length > LIMIT.media[1]!) fail(`${source.media.length} media entries; ${LIMIT.media.join(" to ")}`);
  let total = 0;
  const size = (file: string, limit: number): number => {
    const path = join(OUT, file);
    if (!existsSync(path)) fail(`${file} is not in dist/listing: run without --words`);
    const bytes = statSync(path).size;
    if (bytes > limit) fail(`${file} is ${(bytes / MIB).toFixed(1)} MiB; at most ${limit / MIB}`);
    total += bytes;
    return bytes;
  };
  const probe = async (file: string) => {
    const said = JSON.parse(await $`ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate:format=duration -of json ${join(OUT, file)}`.text());
    return { width: said.streams[0].width as number, height: said.streams[0].height as number, seconds: Number(said.format?.duration ?? 0) };
  };
  const media = [];
  for (const entry of source.media) {
    if (entry.caption.length > LIMIT.caption) fail(`${entry.file}: the caption is ${entry.caption.length} characters; at most ${LIMIT.caption}`);
    size(entry.file, entry.kind === "video" ? LIMIT.clip : LIMIT.still);
    const facts = await probe(entry.file);
    if (entry.kind === "video") {
      size(entry.poster!, LIMIT.still);
      const seconds = Math.round(facts.seconds);
      if (seconds < LIMIT.seconds[0]! || seconds > LIMIT.seconds[1]!) fail(`${entry.file} runs ${facts.seconds.toFixed(1)} s; ${LIMIT.seconds.join(" to ")}`);
      media.push({ kind: entry.kind, file: entry.file, poster: entry.poster, width: facts.width, height: facts.height, seconds, from: "psp", caption: entry.caption });
    } else {
      media.push({ kind: entry.kind, file: entry.file, width: facts.width, height: facts.height, from: "psp", caption: entry.caption });
    }
  }
  size(source.card, LIMIT.card);
  if (total > LIMIT.total) fail(`the listing's files are ${(total / MIB).toFixed(1)} MiB together; at most ${LIMIT.total / MIB}`);
  for (const [lang, said] of Object.entries(source.translations ?? {})) {
    for (const file of Object.keys(said.captions ?? {})) if (!source.media.some((entry) => entry.file === file)) fail(`translations.${lang}.captions names ${file}, which no media entry is`);
  }
  const listing = { tagline: source.tagline, description: source.description, media, card: source.card, ...(source.translations ? { translations: source.translations } : {}) };
  writeFileSync(join(OUT, "listing.json"), JSON.stringify(listing, null, 2) + "\n");
  console.log(`listing: ${OUT}/listing.json, ${media.length} pictures, ${(total / MIB).toFixed(1)} MiB`);
}

if (!flag("--words")) {
  if (!only) rmSync(OUT, { recursive: true, force: true });
  await pictures();
}
await words();

if (flag("--upload")) {
  const drawn: Record<string, string> = existsSync(join(WORK, "drawn.json")) ? JSON.parse(readFileSync(join(WORK, "drawn.json"), "utf8")) : {};
  const files = [...source.media.flatMap((entry) => [entry.file, ...(entry.poster ? [entry.poster] : [])]), source.card];
  const other = files.filter((file) => drawn[file] !== "psp:usb");
  if (other.length > 0) throw new Error(`listing: ${other.join(", ")} ${other.length > 1 ? "were" : "was"} not drawn by a PSP, and the listing says a PSP drew every picture. Film without --emu.`);
  const cli = (process.env.POCKET_STUDIO_CLI ?? "pocket-studio").split(" ");
  await $`${cli} listing ${OUT}`.cwd(ROOT);
}
