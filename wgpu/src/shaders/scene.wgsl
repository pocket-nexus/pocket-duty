// What every program of a frame reads: the frame's matrices, the haze and the ground's shadow.
//
// Matrices are uploaded in row order (`duty_sim::mat`), so a product the device writes M × v is
// written v * M here.
struct Scene {
  // Projection × view, for positions in the world.
  world: mat4x4<f32>,
  // Projection × the view's turn alone, for positions counted from the eye: the sky, the effects.
  eye: mat4x4<f32>,
  // The held weapon's own frustum, for the flash at its muzzle.
  held: mat4x4<f32>,
  haze_colour: vec4<f32>,
  // x: metres from the eye where the haze starts, y: where it is complete, z: 1 when there is haze.
  haze: vec4<f32>,
  // The ground's shadow picture: u = x·a + z·b + c of a place, then v.
  shadow_u: vec4<f32>,
  shadow_v: vec4<f32>,
  // The PS Vita's pack (`rich_fragment`). The sun's depth picture: three rows that take a place
  // (x, y, z, 1) to the picture's u, its v and the place's own depth from the sun.
  sun_u: vec4<f32>,
  sun_v: vec4<f32>,
  sun_w: vec4<f32>,
  // x: what a place's depth may exceed the picture's by before its light is all gone; y: 1 over the
  // depth it goes out across; z: 1 when the ground takes its shadow picture; w: 1 when walls and
  // things take the sun's depth picture.
  sun_edge: vec4<f32>,
  // A muzzle's light: xyz where in the world, w its strength (0: none); and its colour.
  flash: vec4<f32>,
  flash_colour: vec4<f32>,
}
@group(0) @binding(0) var<uniform> scene: Scene;

// How much of a colour is left at a depth, as the GE's fog computes it at each corner.
fn clear_at(depth: f32) -> f32 {
  if (scene.haze.z < 0.5) {
    return 1.0;
  }
  return clamp((scene.haze.y - depth) / (scene.haze.y - scene.haze.x), 0.0, 1.0);
}
