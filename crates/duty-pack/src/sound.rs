//! Sounds: the section `SNDS` is [`Sound`] records, one per id in order, and
//! `SNDD` is their samples, signed 16-bit mono.

/// A sound's samples: `frames` of them from byte `offset` of the sample section.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Sound {
    pub offset: u32,
    pub frames: u32,
    /// Samples a second.
    pub rate: u32,
    /// 1 when it is played end to start without a gap (a bed of ambience).
    pub looped: u32,
}

macro_rules! sounds {
    ($($name:ident),* $(,)?) => {
        /// Every sound the game plays. The number is the record's place in `SNDS`.
        #[repr(u16)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(non_camel_case_types)]
        pub enum Id { $($name),* }
        impl Id {
            pub const ALL: &'static [Id] = &[$(Id::$name),*];
            pub const NAMES: &'static [&'static str] = &[$(stringify!($name)),*];
        }
    };
}

sounds! {
    // The player's weapons, heard from behind the sights: three takes of each so a burst does not repeat one.
    RifleShot0, RifleShot1, RifleShot2,
    SmgShot0, SmgShot1, SmgShot2,
    PistolShot0, PistolShot1, PistolShot2,
    // A soldier's rifle at about 12 m and at about 45 m, two takes of each.
    EnemyShotNear0, EnemyShotNear1, EnemyShotFar0, EnemyShotFar1,
    DryFire,
    // A change of magazine: out, in, the bolt.
    ReloadOut, ReloadIn, ReloadBolt,
    WeaponDraw,
    // A round's impact by surface, two takes of each.
    ImpactConcrete0, ImpactConcrete1, ImpactMetal0, ImpactMetal1, ImpactWood0, ImpactWood1, ImpactDirt0, ImpactDirt1, ImpactFlesh0, ImpactFlesh1,
    // A round passing close.
    Whizz0, Whizz1,
    // A footstep by ground, two takes of each; landing from a jump.
    StepConcrete0, StepConcrete1, StepDirt0, StepDirt1, StepWood0, StepWood1, StepMetal0, StepMetal1,
    Land,
    // What the interface plays for a hit landed, a shot to the head, a kill, a hit taken.
    Hitmarker, Headshot, Kill, Hurt,
    Heartbeat,
    // Soldiers' voices: spotting the player, reloading, hit, dying.
    BarkContact0, BarkContact1, BarkReload, BarkHit, BarkDeath0, BarkDeath1,
    BodyFall,
    Explosion, GrenadeBounce,
    // The street's bed of wind and far-off city, eight seconds end to start.
    Ambience,
    // A distant volley and a distant blast for the ambience's events.
    DistantVolley, DistantBoom,
    // The interface's own: moving a list's mark, choosing a row.
    UiMove, UiSelect,
}
