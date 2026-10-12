//! Deterministic simulation core of Pocket Duty.
//!
//! The same crate runs in the compiler (which casts its rays for the light
//! it bakes, and builds the collision tree the pack carries) and on the
//! device, where it is the game: the collision world, the player, the
//! weapons, the soldiers and the match.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
    feature = "hw-sqrt",
    feature(core_intrinsics),
    allow(internal_features)
)]

extern crate alloc;

pub mod body;
pub mod bvh;
pub mod clip;
pub mod event;
pub mod fastmath;
pub mod fx;
pub mod game;
pub mod hear;
pub mod level;
pub mod mat;
pub mod math;
pub mod mix;
pub mod nav;
pub mod player;
pub mod pose;
pub mod rng;
pub mod soldier;
pub mod tune;
pub mod view;
pub mod weapon;
