// Starts the reference as its own entry does (src/main.js), with the seed its capture harness uses, and
// hands the engine over with its frame loop under the caller's control: a frame happens only inside `pump`.
//
// `map` names a map of ours: a place (maps/<name>/place.js, built by ./placeworld.js with no street), or
// a map in the reference's terms (maps/<name>/layout.js, which the export's server puts in the place of
// the reference's layout, built by ./mapworld.js). Without one the world is the reference's own street.
import { Engine } from '../core/engine.js';
import { createConfig } from '../core/config.js';
import { RenderSystem } from '../render/index.js';
import { MaterialSystem } from '../materials/index.js';
import { SkySystem } from '../sky/index.js';
import { WorldSystem } from '../world/index.js';
import { PhysicsSystem } from '../physics/index.js';
import { PlayerSystem } from '../player/index.js';
import { WeaponSystem } from '../weapons/index.js';
import { FxSystem } from '../fx/index.js';
import { AiSystem } from '../ai/index.js';
import { UiSystem } from '../ui/index.js';
import { AudioSystem } from '../audio/index.js';
import { installShotApi } from '../dev/shots.js';

export async function boot({ quality = 'high', map = '' } = {}) {
  // A place of ours (maps/<map>/place.js), a map in the reference's terms (layout.js), or the street.
  let World = WorldSystem;
  if (map) World = (await import('@map/place')).PLACE ? (await import('./placeworld.js')).PlaceWorldSystem : (await import('./mapworld.js')).MapWorldSystem;
  const config = createConfig({ quality, deterministic: true });
  const canvas = document.getElementById('game');
  const engine = new Engine({ canvas, config });
  engine
    .add(RenderSystem)
    .add(MaterialSystem)
    .add(SkySystem)
    .add(World)
    .add(PhysicsSystem)
    .add(PlayerSystem)
    .add(WeaponSystem)
    .add(FxSystem)
    .add(AiSystem)
    .add(UiSystem)
    .add(AudioSystem);
  await engine.init();
  const shots = installShotApi(engine, { capture: true, lockstep: true });
  engine.start();
  await shots.pump(3);
  window.__ENGINE__ = engine;
  return { engine, pump: shots.pump, system: (id) => engine.registry.get(id) };
}
