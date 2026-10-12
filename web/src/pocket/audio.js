// Every sound the game plays, rendered by the reference's own synthesis → audio.dir.
//
// The reference makes a sound when it is heard: a builder wires oscillators, noise and filters into an audio
// context and the context plays the graph. The handhelds play samples, so each sound of
// crates/duty-pack/src/sound.rs is rendered here once, in an OfflineAudioContext, through the reference's own
// mixer (its buses, their compressors, its reverb, its master chain), connected the way its audio system
// connects that kind of sound. A fixed seed per sound makes two exports the same sound: every draw of the
// reference's generator is the same. The browser's rendering of one graph is not the same to the last bit
// from run to run (two renders differ by about 1e-7, at least 107 dB under a sound's peak).
//
// audio.dir holds one f32 array per sound, named as the sound, and in its header
//   sounds: [{ name, rate, frames, looped, peak, from }]   in the order of sound::Id
// `rate` is the rate the samples play at (the compiler resamples), `from` says which builder made them.
import { Rng } from '../core/rng.js';
import { NoiseBank, airCutoff, biquad, clamp, gain, hit, series } from '../audio/dsp.js';
import { Mixer } from '../audio/mixer.js';
import { SpatialField } from '../audio/spatial.js';
import { Ambience } from '../audio/ambience.js';
import { WEAPON_PROFILES, weaponShot, bulletWhizz, dryFire } from '../audio/weapons.js';
import { surfaceImpact, footstep, reloadPhase, explosion, bodyFall, uiSound, heartbeat, cloth } from '../audio/foley.js';
import { bark } from '../audio/vox.js';

const RATE = 48000;
const SEED = 0xd07e5eed;
// The reverbs' impulse responses are drawn from the mixer's generator: one seed, so every sound is heard in
// the same street.
const ROOM_SEED = 0x57ee7;
// Seconds before a voice starts: the envelopes need a moment of timeline, and a bark's consonant leads its
// vowel by up to 55 ms.
const LEAD = 0.02;
const BARK_LEAD = 0.06;

// Where the listener stands, as the weights of the reference's five reverbs. Its space probe (nine rays,
// classifySpace) gives street 0.72…0.94 and open 0.28…0.06 at six of the level's eight spawn points; this
// is their middle: mostly the street's slap, some of the open ground's roll.
const SPACE = { tight: 0, room: 0, street: 0.8, tunnel: 0, open: 0.2 };
// How much of a shot the reference sends to the reverb in that space: AudioSystem._build, and _onFire for
// the player's own weapon.
const ECHO_BOOST = 0.75 + SPACE.street * 0.7 + SPACE.tight * 0.35 + SPACE.tunnel * 0.8 + SPACE.open * 0.2;
const OWN_ECHO = (0.35 + SPACE.tight * 0.5 + SPACE.street * 0.9 + SPACE.tunnel * 1.0 + SPACE.room * 0.75) * 0.6;

// The reference's own curve of level over distance (it reads nothing of the field it belongs to).
const attenuation = (dist) => SpatialField.prototype.attenuation(dist);

/** A voice with no place in the world (the player's own weapon, the interface): AudioSystem._playDry. */
function dry(mixer, voice, bus, send) {
  voice.node.connect(mixer.bus(bus));
  const level = send * (voice.send ?? 1);
  if (level > 0.001) {
    const g = gain(mixer.actx, level);
    voice.node.connect(g);
    g.connect(mixer.reverbSend);
  }
}

/**
 * A voice `dist` metres away with nothing in between: AudioSystem._playAt and the emitter of
 * SpatialField.acquire (the air's low-pass, the level over distance, a reverb send taken after it so a far
 * sound is wetter). The emitter's panner is left out: the device places a sound between its speakers.
 */
function placed(mixer, voice, bus, dist, o = {}) {
  const actx = mixer.actx;
  const air = biquad(actx, 'lowpass', airCutoff(dist), 0.5);
  const level = gain(actx, clamp(attenuation(dist) * (o.gain ?? 1), 0, 4));
  series(voice.node, air, level).connect(mixer.bus(bus));
  const send = gain(actx, clamp((o.send ?? voice.send ?? 0.3) * (0.5 + Math.min(dist, 90) * 0.022), 0, 3));
  level.connect(send);
  send.connect(mixer.reverbSend);
}

/**
 * The reference has no sound for a menu. A tick is assembled from its helpers in the manner of its interface
 * sounds (short, dry, not a recording of anything): its noise through a band-pass under its `hit` envelope.
 */
function tick(actx, bank, rng, o) {
  const out = gain(actx, 1);
  const src = bank.source('white', rng, 1);
  const bp = biquad(actx, 'bandpass', o.f, o.q);
  const g = gain(actx, 0);
  series(src, bp, g).connect(out);
  hit(g.gain, o.when, o.level, o.decay);
  src.start(o.when, src._offset, o.decay + 0.02);
  return { node: out, end: o.when + o.decay + 0.05, send: 0 };
}

// A profile carries its table of round-robin variants once a shot was built from it. Copies of the three
// the game uses, without a table, so the table comes from this export's seed alone.
const profile = ({ _rr, _rrIndex, ...p }) => p;
const profiles = { rifle: profile(WEAPON_PROFILES.rifle), smg: profile(WEAPON_PROFILES.smg), pistol: profile(WEAPON_PROFILES.pistol) };

// A soldier's voice as AudioSystem.bark derives it from the soldier's number.
const voiceOf = (id) => ({ f0: 96 + ((id * 37) % 41), tract: 0.95 + ((id * 13) % 11) / 100 });

// ---------------------------------------------------------------- the sounds
// `keep` is the most seconds of a sound that are sent, counted from where it starts, and `fade` the seconds
// its end fades over; `seconds` is how long the context renders.

/** The player's weapon from behind the sights: no distance, the head-locked path of AudioSystem._onFire. */
const ownShot = (key) => ({
  from: `weaponShot ${key}, first person, 0 m`, seconds: 1.4, keep: 0.9, fade: 0.12,
  build: ({ actx, rng, bank, mixer, t }) => dry(mixer, weaponShot(actx, bank, rng, profiles[key], { when: t, distance: 0, firstPerson: true, echoBoost: ECHO_BOOST }), 'weapons', OWN_ECHO),
});

/** A soldier's rifle (the reference resolves `ai_rifle` to the rifle profile) at `dist` metres. */
const soldierShot = (dist) => ({
  from: `weaponShot rifle, ${dist} m`, seconds: 1.6, keep: 0.9, fade: 0.12,
  build: ({ actx, rng, bank, mixer, t }) => placed(mixer, weaponShot(actx, bank, rng, profiles.rifle, { when: t, distance: dist, firstPerson: false, echoBoost: ECHO_BOOST }), 'weapons', dist),
});

const reload = (phase, keep) => ({
  from: `reloadPhase ${phase}`, seconds: keep + 0.6, keep, fade: 0.08,
  build: ({ actx, rng, bank, mixer, t }) => dry(mixer, reloadPhase(actx, bank, rng, phase, { when: t, heavy: 1 }), 'foley', 0.22),
});

/** A round's impact 6 m away, at the energy of a rifle round (damage 34). */
const impact = (surface) => ({
  from: `surfaceImpact ${surface}, 6 m`, seconds: 1.4, keep: 0.6, fade: 0.1,
  build: ({ actx, rng, bank, mixer, t }) => placed(mixer, surfaceImpact(actx, bank, rng, { when: t, surface, energy: 1 }), 'foley', 6),
});

/** A round passing `miss` metres from the ear. */
const whizz = (miss) => ({
  from: `bulletWhizz, ${miss} m from the ear`, seconds: 1.0, keep: 0.35, fade: 0.08,
  build: ({ actx, rng, bank, mixer, t }) => placed(mixer, bulletWhizz(actx, bank, rng, { when: t, miss }), 'foley', miss),
});

/** The player's own step: 1.6 m under the ear, at the level the reference gives a step that close. */
const step = (surface) => ({
  from: `footstep ${surface}, walk`, seconds: 1.2, keep: 0.4, fade: 0.08,
  build: ({ actx, rng, bank, mixer, t }) => placed(mixer, footstep(actx, bank, rng, { when: t, surface, gait: 'walk', level: 0.72 }), 'foley', 1.6),
});

const ui = (kind, keep) => ({
  from: `uiSound ${kind}`, seconds: keep + 0.3, keep, fade: 0.03,
  build: ({ actx, rng, bank, mixer, t }) => dry(mixer, uiSound(actx, bank, rng, kind, { when: t, level: 1 }), 'ui', 0),
});

/** A soldier 10 m away calling out. */
const call = (script, id, keep) => ({
  from: `bark ${script}, voice ${id}, 10 m`, seconds: keep + 0.8, keep, fade: 0.15, lead: BARK_LEAD,
  build: ({ actx, rng, bank, mixer, t }) => placed(mixer, bark(actx, bank, rng, { when: t, bark: script, ...voiceOf(id), level: 1, radio: false }), 'voice', 10),
});

const menuTick = (f, decay, level) => ({
  from: `assembled: bank noise → band-pass ${f} Hz → hit envelope ${decay * 1000} ms`, seconds: 0.5, keep: decay + 0.06, fade: 0.02,
  build: ({ actx, rng, bank, mixer, t }) => dry(mixer, tick(actx, bank, rng, { when: t, f, q: 3, decay, level }), 'ui', 0),
});

// crates/duty-pack/src/sound.rs, in its order.
const SOUNDS = [
  ['RifleShot0', ownShot('rifle')], ['RifleShot1', ownShot('rifle')], ['RifleShot2', ownShot('rifle')],
  ['SmgShot0', ownShot('smg')], ['SmgShot1', ownShot('smg')], ['SmgShot2', ownShot('smg')],
  ['PistolShot0', ownShot('pistol')], ['PistolShot1', ownShot('pistol')], ['PistolShot2', ownShot('pistol')],
  ['EnemyShotNear0', soldierShot(12)], ['EnemyShotNear1', soldierShot(12)], ['EnemyShotFar0', soldierShot(45)], ['EnemyShotFar1', soldierShot(45)],
  ['DryFire', {
    from: 'dryFire', seconds: 0.8, keep: 0.3, fade: 0.05,
    build: ({ actx, rng, bank, mixer, t }) => dry(mixer, dryFire(actx, bank, rng, { when: t }), 'weapons', 0.15),
  }],
  ['ReloadOut', reload('magout', 0.7)], ['ReloadIn', reload('magin', 0.5)], ['ReloadBolt', reload('end', 0.6)],
  // Raising a weapon: the sleeve, then the hand closing on the magazine well (the reload's first phase).
  ['WeaponDraw', {
    from: 'cloth, then reloadPhase start 50 ms later', seconds: 1.2, keep: 0.6, fade: 0.08,
    build: ({ actx, rng, bank, mixer, t }) => {
      dry(mixer, cloth(actx, bank, rng, { when: t, level: 0.9 }), 'foley', 0.12);
      dry(mixer, reloadPhase(actx, bank, rng, 'start', { when: t + 0.05, heavy: 1 }), 'foley', 0.22);
    },
  }],
  ['ImpactConcrete0', impact('concrete')], ['ImpactConcrete1', impact('concrete')], ['ImpactMetal0', impact('metal')], ['ImpactMetal1', impact('metal')],
  ['ImpactWood0', impact('wood')], ['ImpactWood1', impact('wood')], ['ImpactDirt0', impact('dirt')], ['ImpactDirt1', impact('dirt')],
  ['ImpactFlesh0', impact('flesh')], ['ImpactFlesh1', impact('flesh')],
  ['Whizz0', whizz(0.7)], ['Whizz1', whizz(1.8)],
  ['StepConcrete0', step('concrete')], ['StepConcrete1', step('concrete')], ['StepDirt0', step('dirt')], ['StepDirt1', step('dirt')],
  ['StepWood0', step('wood')], ['StepWood1', step('wood')], ['StepMetal0', step('metal')], ['StepMetal1', step('metal')],
  // A landing at 7 m/s, with the gear's rattle: AudioSystem._onLand.
  ['Land', {
    from: 'footstep concrete, land', seconds: 1.4, keep: 0.55, fade: 0.1,
    build: ({ actx, rng, bank, mixer, t }) => placed(mixer, footstep(actx, bank, rng, { when: t, surface: 'concrete', gait: 'land', level: 1, gear: 1 }), 'foley', 1.6),
  }],
  ['Hitmarker', ui('hitmarker', 0.15)], ['Headshot', ui('headshot', 0.25)], ['Kill', ui('kill', 0.45)], ['Hurt', ui('damage', 0.5)],
  ['Heartbeat', {
    from: 'heartbeat', seconds: 1.2, keep: 0.6, fade: 0.08,
    build: ({ actx, rng, bank, mixer, t }) => dry(mixer, heartbeat(actx, bank, rng, { when: t, level: 1 }), 'foley', 0.1),
  }],
  ['BarkContact0', call('contact', 3, 0.8)], ['BarkContact1', call('spotted', 7, 1.1)], ['BarkReload', call('reloading', 5, 0.9)],
  ['BarkHit', call('hit', 2, 0.6)], ['BarkDeath0', call('death', 4, 1.8)], ['BarkDeath1', call('death', 8, 1.8)],
  ['BodyFall', {
    from: 'bodyFall, 10 m', seconds: 1.6, keep: 0.7, fade: 0.1,
    build: ({ actx, rng, bank, mixer, t }) => placed(mixer, bodyFall(actx, bank, rng, { when: t, level: 1 }), 'foley', 10),
  }],
  // A soldier's grenade (radius 6.5 m) 15 m away: AudioSystem._onExplosion, without the deafening it puts
  // on the whole mix, which is the listener's state and not this sound's.
  ['Explosion', {
    from: 'explosion radius 6.5 m, 15 m', seconds: 3.4, keep: 2.4, fade: 0.4,
    build: ({ actx, rng, bank, mixer, t }) => placed(mixer, explosion(actx, bank, rng, { when: t, distance: 15, radius: 6.5, level: 1 }), 'weapons', 15, { send: 1 }),
  }],
  // The reference's grenade is silent until it bursts. A steel body on the street is its round's impact on
  // metal, played 0.55 times as fast (the samples are sent as they were rendered, with a lower rate).
  ['GrenadeBounce', {
    from: 'surfaceImpact metal, 8 m, played at 0.55 of its rate', seconds: 1.2, keep: 0.33, fade: 0.06, speed: 0.55,
    build: ({ actx, rng, bank, mixer, t }) => placed(mixer, surfaceImpact(actx, bank, rng, { when: t, surface: 'metal', energy: 0.7 }), 'foley', 8),
  }],
  // The beds the reference plays without end outdoors: two layers of wind, its whistle, the city's hum, the
  // war's rumble. Their slow oscillators start within its first ten seconds; the loop is cut after them.
  ['Ambience', {
    from: 'Ambience.start, 10.5 s in, the last 0.5 s faded into the first', seconds: 19, loop: { from: 10.5, seconds: 8, cross: 0.5 },
    build: ({ actx, rng, bank, mixer }) => new Ambience(actx, bank, mixer, null, rng.fork()).start(),
  }],
  // Four rounds 160 m away, over the roofs: AudioSystem._distantVolley with the rifle.
  ['DistantVolley', {
    from: 'weaponShot rifle × 4, 160 m', seconds: 4.2, keep: 3.0, fade: 0.6,
    build: ({ actx, rng, bank, mixer, t }) => {
      const spacing = rng.range(0.075, 0.13);
      for (let i = 0; i < 4; i++) {
        const when = t + i * spacing * rng.range(0.9, 1.1);
        placed(mixer, weaponShot(actx, bank, rng, profiles.rifle, { when, distance: 160, firstPerson: false, echoBoost: ECHO_BOOST }), 'weapons', 160, { gain: 4.5 });
      }
    },
  }],
  // AudioSystem._distantBoom at the middle of its radii.
  ['DistantBoom', {
    from: 'explosion radius 11 m, 200 m', seconds: 5.4, keep: 4.0, fade: 1.0,
    build: ({ actx, rng, bank, mixer, t }) => placed(mixer, explosion(actx, bank, rng, { when: t, distance: 200, radius: 11, level: 1 }), 'weapons', 200, { gain: 6 }),
  }],
  ['UiMove', menuTick(3400, 0.012, 0.5)], ['UiSelect', menuTick(2300, 0.045, 0.6)],
];

// ---------------------------------------------------------------- rendering
/** One sound: a context, the reference's noise and mixer in it, the sound's voices, and what came out. */
async function render(spec, seed) {
  const actx = new OfflineAudioContext(2, Math.ceil(RATE * spec.seconds), RATE);
  const rng = new Rng(seed);
  const bank = new NoiseBank(actx, rng.fork(), 2.4);
  const mixer = new Mixer(actx, new Rng(ROOM_SEED), {});
  Object.assign(mixer.spaceWeights, SPACE);
  mixer.buildReverbs();
  spec.build({ actx, rng, bank, mixer, t: spec.lead ?? LEAD });
  const buffer = await actx.startRendering();
  const left = buffer.getChannelData(0), right = buffer.getChannelData(1);
  if (spec.loop) return loop(left, right, spec.loop);
  // One ear's signal: the two channels differ only in their noise and their reverb, and a mean of them
  // would lower both against what is the same in each.
  return cut(left, spec);
}

/** From where the sound starts (60 dB under its peak) for at most `keep` seconds, the end faded out. */
function cut(samples, spec) {
  let peak = 0;
  for (const v of samples) peak = Math.max(peak, Math.abs(v));
  const floor = peak * 0.001;
  let start = 0;
  while (start < samples.length && Math.abs(samples[start]) < floor) start++;
  // A millisecond before it, for the rise of the first wave.
  start = Math.max(0, start - Math.round(RATE * 0.001));
  const out = samples.slice(start, Math.min(samples.length, start + Math.round(RATE * spec.keep)));
  const fade = Math.min(out.length, Math.round(RATE * spec.fade));
  for (let i = 0; i < fade; i++) out[out.length - 1 - i] *= 0.5 - 0.5 * Math.cos((Math.PI * i) / fade);
  return out;
}

/**
 * `seconds` of a bed that plays end to start: the two channels' mean (its layers stand left and right),
 * and the `cross` seconds after the end faded into the start at equal power (two stretches of noise).
 */
function loop(left, right, { from, seconds, cross }) {
  const at = Math.round(RATE * from), n = Math.round(RATE * seconds), x = Math.round(RATE * cross);
  const mid = (i) => 0.5 * (left[at + i] + right[at + i]);
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) out[i] = mid(i);
  for (let i = 0; i < x; i++) {
    const a = (Math.PI / 2) * ((i + 0.5) / x);
    out[i] = mid(i) * Math.sin(a) + mid(n + i) * Math.cos(a);
  }
  return out;
}

export async function exportAudio(ctx) {
  const bundle = new ctx.Bundle({ rate: RATE, seed: SEED, space: SPACE, sounds: [] });
  let seconds = 0;
  for (const [i, [name, spec]] of SOUNDS.entries()) {
    const samples = await render(spec, (SEED + i * 7919) >>> 0);
    let peak = 0;
    for (const v of samples) {
      if (!Number.isFinite(v)) throw new Error(`audio: ${name} has a sample that is not a number`);
      peak = Math.max(peak, Math.abs(v));
    }
    if (!(peak > 1e-5)) throw new Error(`audio: ${name} is silent`);
    const rate = Math.round(RATE * (spec.speed ?? 1));
    bundle.meta.sounds.push({ name, rate, frames: samples.length, looped: !!spec.loop, peak, from: spec.from });
    bundle.add(name, samples);
    seconds += samples.length / rate;
    ctx.log(`audio: ${name} ${(samples.length / rate).toFixed(2)} s, peak ${(20 * Math.log10(peak)).toFixed(1)} dB (${spec.from})`);
  }
  await ctx.put('audio.dir', bundle.blob());
  ctx.log(`audio: ${SOUNDS.length} sounds, ${seconds.toFixed(1)} s, ${(bundle.bytes / 1e6).toFixed(1)} MB at ${RATE} Hz`);
}
