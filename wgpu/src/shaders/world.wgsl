// The world, the sky and the effects, as the PSP's GE draws them with no program at all
// (psp/src/gfx.rs): a texel times the corner's colour, doubled for the world, then the haze.

@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var texels: sampler;

// ---------------------------------------------------------------- the street
// A corner is its cell's centre plus a whole number of lattice steps; its colour is the surface
// under the baked light for a texel of one half, so the texture stage doubles the product.

struct Chunk {
  // The cell's centre, and metres a step of a stored position is.
  place: vec4<f32>,
  // The texture coordinates' scale (xy) and offset (zw).
  uv: vec4<f32>,
  // x: 1 when the picture's clear texels are holes.
  kind: vec4<f32>,
}
@group(2) @binding(0) var<uniform> chunk: Chunk;

struct WorldOut {
  @builtin(position) @invariant position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) colour: vec3<f32>,
  @location(2) clear: f32,
  // Where the corner stands in the ground's shadow picture.
  @location(3) ground: vec2<f32>,
}

@vertex
fn world_vertex(@location(0) position: vec4<i32>, @location(1) uv: vec2<i32>, @location(2) colour: vec4<f32>) -> WorldOut {
  var out: WorldOut;
  let at = chunk.place.xyz + vec3<f32>(position.xyz) * chunk.place.w;
  let clip = vec4<f32>(at, 1.0) * scene.world;
  out.position = clip;
  out.uv = vec2<f32>(uv) / 32768.0 * chunk.uv.xy + chunk.uv.zw;
  out.colour = colour.rgb;
  out.clear = clear_at(clip.w);
  out.ground = vec2<f32>(at.x * scene.shadow_u.x + at.z * scene.shadow_u.y + scene.shadow_u.z, at.x * scene.shadow_v.x + at.z * scene.shadow_v.y + scene.shadow_v.z);
  return out;
}

@fragment
fn world_fragment(in: WorldOut) -> @location(0) vec4<f32> {
  let texel = textureSample(picture, texels, in.uv);
  // (the GE's alpha test: more than a half passes)
  if (chunk.kind.x > 0.5 && texel.a <= 0.5) {
    discard;
  }
  let lit = min(texel.rgb * in.colour * 2.0, vec3<f32>(1.0));
  return vec4<f32>(mix(scene.haze_colour.rgb, lit, in.clear), 1.0);
}

// The ground once more: what is there times the shadow picture, whose texel of one half is all of the sun.
@fragment
fn shadow_fragment(in: WorldOut) -> @location(0) vec4<f32> {
  return vec4<f32>(min(textureSample(picture, texels, in.ground).rgb * 2.0, vec3<f32>(1.0)), 1.0);
}

// ---------------------------------------------------------------- squares counted from the eye
// The sky's band and its cap, and the effects: a texture coordinate, a colour, a position.

struct FlatOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) colour: vec4<f32>,
}

@vertex
fn eye_vertex(@location(0) uv: vec2<f32>, @location(1) colour: vec4<f32>, @location(2) position: vec3<f32>) -> FlatOut {
  var out: FlatOut;
  out.position = vec4<f32>(position, 1.0) * scene.eye;
  out.uv = uv;
  out.colour = colour;
  return out;
}

// In the held weapon's own space and frustum: the flash at its muzzle.
@vertex
fn held_vertex(@location(0) uv: vec2<f32>, @location(1) colour: vec4<f32>, @location(2) position: vec3<f32>) -> FlatOut {
  var out: FlatOut;
  out.position = vec4<f32>(position, 1.0) * scene.held;
  out.uv = uv;
  out.colour = colour;
  return out;
}

// A picture as it is: the sky's band.
@fragment
fn picture_fragment(in: FlatOut) -> @location(0) vec4<f32> {
  return vec4<f32>(textureSample(picture, texels, in.uv).rgb * in.colour.rgb, 1.0);
}

// A colour alone: the sky above the pictures.
@fragment
fn colour_fragment(in: FlatOut) -> @location(0) vec4<f32> {
  return vec4<f32>(in.colour.rgb, 1.0);
}

// A shape of the effects' picture in the corner's colour, as opaque as both say.
@fragment
fn effect_fragment(in: FlatOut) -> @location(0) vec4<f32> {
  return textureSample(picture, texels, in.uv) * in.colour;
}
