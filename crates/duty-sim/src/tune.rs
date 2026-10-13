//! The numbers the game runs on.
//!
//! The player's and the soldiers' are the reference's (`web/src/player/tuning.js`,
//! `web/src/ai/agent.js`), written here with where each was read. The weapons'
//! numbers and the surfaces' travel in the pack: the export reads them from
//! the running reference and the compiler stores them as the records below.

/// Simulation steps a second. The reference steps at 120; the handheld takes half as many, and
/// every rate here is per second, so the motion is the same.
pub const TICKS: f32 = 60.0;
pub const DT: f32 = 1.0 / TICKS;

/// `core/config.js:14`: metres per second squared, on the player and everything thrown.
pub const GRAVITY: f32 = 9.81 * 2.1;

pub mod player {
    // `player/tuning.js:27-52`, stance by stance: the capsule's height, the eye's, the speed.
    pub const RADIUS: f32 = 0.32;
    pub const HEIGHT: [f32; 2] = [1.78, 1.12];
    pub const EYE: [f32; 2] = [1.66, 1.02];
    pub const SPEED: [f32; 2] = [4.57, 2.44];
    pub const STEP: [f32; 2] = [0.42, 0.30];
    pub const STRIDE: [f32; 2] = [1.48, 1.05];
    // `player/tuning.js:54-89`
    pub const SPRINT_SPEED: f32 = 7.01;
    pub const STRAFE_SCALE: f32 = 0.92;
    pub const BACK_SCALE: f32 = 0.8;
    pub const ADS_SCALE: f32 = 0.5;
    pub const GROUND_ACCEL: f32 = 92.0;
    pub const GROUND_DECEL: f32 = 52.0;
    pub const STOP_DECEL: f32 = 30.0;
    pub const AIR_ACCEL: f32 = 23.0;
    pub const AIR_SPEED_CAP: f32 = 3.4;
    pub const TERMINAL: f32 = 55.0;
    /// The jump's height, and the speed that reaches it under `GRAVITY`.
    pub const JUMP_APEX: f32 = 0.6;
    pub const COYOTE: f32 = 0.09;
    pub const JUMP_BUFFER: f32 = 0.13;
    pub const JUMP_COOLDOWN: f32 = 0.28;
    /// Sprint needs the stick this far forward of sideways (`sprintForwardDot`).
    pub const SPRINT_FORWARD: f32 = 0.55;
    /// The cosine of the steepest slope that is still ground: 48 degrees.
    pub const SLOPE: f32 = 0.669;
    pub const SNAP: f32 = 0.34;
    // `player/tuning.js:91-111`: the slide.
    pub const SLIDE_ENTRY: f32 = 8.84;
    pub const SLIDE_MIN_ENTRY: f32 = 6.2;
    pub const SLIDE_EXIT: f32 = 2.95;
    pub const SLIDE_TIME: f32 = 0.9;
    pub const SLIDE_COOLDOWN: f32 = 0.55;
    pub const SLIDE_START: f32 = 5.2;
    // `player/tuning.js:113-136`: climbing onto and over things.
    pub const MANTLE_MIN: f32 = 0.5;
    pub const MANTLE_MAX: f32 = 1.85;
    pub const MANTLE_REACH: f32 = 0.62;
    /// This game's: metres a second up or down a ladder, how near its line he takes hold of it,
    /// and how far under its top he is carried onto the floor.
    pub const LADDER_SPEED: f32 = 2.6;
    pub const LADDER_REACH: f32 = 0.55;
    pub const LADDER_HEAD: f32 = 0.6;
    // `player/tuning.js:148-152`: seconds the eye takes to settle after a change of stance.
    pub const EYE_TAU: f32 = 0.067;
    // `player/tuning.js:255-265`
    pub const HEALTH: f32 = 100.0;
    pub const REGEN_DELAY: f32 = 4.6;
    pub const REGEN_RATE: f32 = 34.0;
    pub const REGEN_RAMP: f32 = 0.55;
    pub const FALL_SAFE: f32 = 15.0;
    pub const FALL_DAMAGE: f32 = 7.0;
    // `core/config.js:86-90`, `player/camera.js:305`: the view.
    pub const ADS_FOV: f32 = 0.72;
    pub const ADS_LOOK: f32 = 0.65;
    pub const SPRINT_FOV: f32 = 1.055;
    pub const PITCH_LIMIT: f32 = 1.536;
    // `player/tuning.js:162-172`: the walk's bob, metres and radians.
    pub const BOB: [f32; 3] = [0.0165, 0.0115, 0.006];
    /// `player/index.js:240-245`: radians a second a stick turns the view at full deflection.
    pub const TURN: f32 = 3.1;
}

pub mod soldier {
    // `ai/agent.js:122-217`
    pub const HEALTH: f32 = 100.0;
    pub const RADIUS: f32 = 0.34;
    pub const HEIGHT: f32 = 1.78;
    pub const EYE: f32 = 1.665;
    pub const VIEW_RANGE: f32 = 58.0;
    pub const WEAPON_RANGE: f32 = 60.0;
    /// Rounds a second: the carbine, the irregular's rifle.
    pub const FIRE_RATE: [f32; 2] = [10.5, 8.2];
    pub const RELOAD: [f32; 2] = [2.35, 2.9];
    pub const MAGAZINE: u32 = 30;
    pub const SPREAD: f32 = 0.032;
    pub const DAMAGE: f32 = 17.0;
    /// `ai/index.js:628-655`: a round passing this near the player's chest hits; nearer than the second, harder.
    pub const HIT_RADIUS: f32 = 0.42;
    pub const HIT_CLOSE: f32 = 0.16;
    pub const CHEST: f32 = 1.35;
    // `ai/agent.js:363-445`: metres a second in each state.
    pub const PATROL_SPEED: f32 = 1.35;
    pub const ALERT_SPEED: f32 = 1.5;
    pub const COVER_SPEED: f32 = 4.3;
    pub const FLANK_SPEED: f32 = 4.4;
    pub const RETREAT_SPEED: f32 = 4.6;
    /// This game's: metres a second up or down a ladder, whatever his state.
    pub const LADDER_SPEED: f32 = 1.6;
    // `ai/index.js:352-359`: the player's rounds lose force with distance on the soldiers' side too.
    pub const FALLOFF_FROM: f32 = 22.0;
    pub const FALLOFF_PER_METRE: f32 = 0.0125;
    pub const FALLOFF_FLOOR: f32 = 0.45;
    // `ai/index.js:672-717`: the grenade.
    pub const GRENADE_FUSE: f32 = 2.35;
    pub const GRENADE_RADIUS: f32 = 6.5;
    pub const GRENADE_DAMAGE: f32 = 120.0;
    /// `ai/agent.js:40-48`: the hit zones' radii and what a hit there is worth: head, upper torso,
    /// lower torso, arm, leg.
    pub const ZONE_RADIUS: [f32; 5] = [0.098, 0.185, 0.175, 0.072, 0.105];
    pub const ZONE_SCALE: [f32; 5] = [4.0, 1.0, 0.9, 0.65, 0.7];
}

/// A weapon's fire modes.
pub mod mode {
    pub const AUTO: u32 = 0;
    pub const BURST: u32 = 1;
    pub const SEMI: u32 = 2;
    pub const NAMES: [&str; 3] = ["AUTO", "BURST", "SEMI"];
}

/// Shots a recoil pattern holds at most.
pub const PATTERN: usize = 32;

/// One of the three weapons, as `web/src/weapons/defs.js` defines it. The pack's `WEAP` section
/// is three of these: rifle, submachine gun, pistol.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct WeaponDef {
    /// The name the interface shows, NUL-padded.
    pub label: [u8; 12],
    /// The fire modes it has, the one it starts in first; `modes` of them.
    pub mode: [u32; 3],
    pub modes: u32,
    /// Rounds a minute, in bursts, and seconds between two bursts.
    pub rpm: f32,
    pub burst_rpm: f32,
    pub burst_delay: f32,
    pub burst_count: u32,
    pub magazine: u32,
    pub reserve: u32,
    pub damage: f32,
    /// What is left of the damage at `max_range`, falling with the square of the distance.
    pub dropoff: f32,
    pub max_range: f32,
    /// How deep a round goes: a surface's `pen_depth` times this is the wall it passes.
    pub penetration: f32,
    pub tracer_every: u32,
    /// Degrees: at the hip, down the sights, added by a shot, at most, and lost a second.
    pub spread_hip: f32,
    pub spread_ads: f32,
    pub spread_shot: f32,
    pub spread_max: f32,
    pub spread_decay: f32,
    /// The kick a shot gives the held weapon: back and up in metres, roll, and the view's punch.
    pub kick_back: f32,
    pub kick_up: f32,
    pub kick_roll: f32,
    pub punch: f32,
    /// Seconds to raise the sights, to change a part-used magazine, an empty one, to draw, to lower.
    pub ads_time: f32,
    pub reload_tac: f32,
    pub reload_empty: f32,
    pub draw_time: f32,
    pub holster_time: f32,
    /// The view's climb and drift for each shot of a burst, radians: pitch, yaw.
    pub pattern_length: u32,
    pub pattern: [[f32; 2]; PATTERN],
}

/// A surface a round can meet (`web/src/physics/surfaces.js:44-69`). The pack's collision triangles
/// carry the surface's number in their tag's low byte.
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub name: &'static str,
    /// Metres of it a round of penetration 1 passes.
    pub pen_depth: f32,
    /// What passing all of that costs the round's damage.
    pub energy_loss: f32,
    pub friction: f32,
}

pub const SURFACES: [Surface; 12] = [
    Surface {
        name: "concrete",
        pen_depth: 0.055,
        energy_loss: 0.62,
        friction: 0.92,
    },
    Surface {
        name: "metal",
        pen_depth: 0.022,
        energy_loss: 0.70,
        friction: 0.52,
    },
    Surface {
        name: "wood",
        pen_depth: 0.32,
        energy_loss: 0.30,
        friction: 0.72,
    },
    Surface {
        name: "dirt",
        pen_depth: 0.26,
        energy_loss: 0.45,
        friction: 0.96,
    },
    Surface {
        name: "sand",
        pen_depth: 0.19,
        energy_loss: 0.55,
        friction: 1.05,
    },
    Surface {
        name: "glass",
        pen_depth: 0.45,
        energy_loss: 0.12,
        friction: 0.32,
    },
    Surface {
        name: "water",
        pen_depth: 1.1,
        energy_loss: 0.50,
        friction: 0.30,
    },
    Surface {
        name: "foliage",
        pen_depth: 3.0,
        energy_loss: 0.05,
        friction: 0.62,
    },
    Surface {
        name: "fabric",
        pen_depth: 2.2,
        energy_loss: 0.06,
        friction: 0.80,
    },
    Surface {
        name: "flesh",
        pen_depth: 0.55,
        energy_loss: 0.35,
        friction: 0.90,
    },
    Surface {
        name: "rubber",
        pen_depth: 0.28,
        energy_loss: 0.40,
        friction: 1.25,
    },
    Surface {
        name: "plaster",
        pen_depth: 0.70,
        energy_loss: 0.12,
        friction: 0.86,
    },
];

pub mod surface {
    pub const CONCRETE: u8 = 0;
    pub const METAL: u8 = 1;
    pub const WOOD: u8 = 2;
    pub const DIRT: u8 = 3;
    pub const SAND: u8 = 4;
    pub const FLESH: u8 = 9;
}
