// The soldiers and the held weapon: four bones blended and two lights, which the PSP's GE does
// in its vertex stage with no program (psp/src/figures.rs).

struct Figure {
  // The figure's own space to the screen.
  clip: mat4x4<f32>,
  // Four bones, each three rows of four.
  bones: array<vec4<f32>, 12>,
  // In the figure's own space: toward the sun, the sun's colour where the figure stands, up, the sky's colour.
  sun: vec4<f32>,
  sun_colour: vec4<f32>,
  up: vec4<f32>,
  sky: vec4<f32>,
}
@group(1) @binding(0) var<uniform> figure: Figure;

struct SkinOut {
  @builtin(position) position: vec4<f32>,
  @location(0) colour: vec3<f32>,
}

@vertex
fn skin_vertex(@location(0) weights: vec4<u32>, @location(1) colour: vec4<f32>, @location(2) normal: vec4<i32>, @location(3) position: vec3<f32>) -> SkinOut {
  // A weight of 128 is all of a bone.
  let share = vec4<f32>(weights) / 128.0;
  let p = vec4<f32>(position, 1.0);
  let n = vec4<f32>(vec3<f32>(normal.xyz) / 127.0, 0.0);
  var at = vec3<f32>(0.0);
  var facing = vec3<f32>(0.0);
  for (var b = 0u; b < 4u; b++) {
    let x = figure.bones[b * 3u];
    let y = figure.bones[b * 3u + 1u];
    let z = figure.bones[b * 3u + 2u];
    at += share[b] * vec3<f32>(dot(x, p), dot(y, p), dot(z, p));
    facing += share[b] * vec3<f32>(dot(x, n), dot(y, n), dot(z, n));
  }
  facing = normalize(facing);
  var out: SkinOut;
  out.position = vec4<f32>(at, 1.0) * figure.clip;
  // The sky's fill comes from above more than from the sides: 0.55 of it from everywhere and 0.6
  // of it more on what faces up, so that a figure in the shade still has a form.
  let light = figure.sky.rgb * 0.55 + figure.sun_colour.rgb * max(dot(facing, figure.sun.xyz), 0.0) + figure.sky.rgb * 0.6 * max(dot(facing, figure.up.xyz), 0.0);
  out.colour = min(colour.rgb * light, vec3<f32>(1.0));
  return out;
}

@fragment
fn skin_fragment(in: SkinOut) -> @location(0) vec4<f32> {
  return vec4<f32>(in.colour, 1.0);
}
