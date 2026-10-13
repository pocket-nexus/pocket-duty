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
  // x: 1 when the picture's clear texels are holes. In the PS Vita's pack also y: 1 for ground, which
  // takes the sun's shadow from the ground's picture, and z: 1 for what keeps the sun's light apart
  // in its corners and takes the shadow from the sun's depth picture.
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

// ---------------------------------------------------------------- the street of the PS Vita's pack
// As the console's own programs draw it (vita/shaders/world_v.cg, world_f.cg): the ground's shadow in
// the same pass, the sun's shadow on walls and things a pixel at a time from its depth picture, a
// muzzle's light on what is near it.

@group(3) @binding(0) var ground_shadow: texture_2d<f32>;
@group(3) @binding(1) var ground_texels: sampler;
// The sun's depths, 16 bits a texel: whole numbers, mixed between four texels here as the console's
// GPU mixes them.
@group(3) @binding(2) var sun_depths: texture_2d<u32>;

struct RichOut {
  @builtin(position) @invariant position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  // The corner's colour with nothing between it and the sun, and with the sun off.
  @location(1) colour: vec3<f32>,
  @location(2) shade: vec3<f32>,
  @location(3) clear: f32,
  @location(4) ground: vec2<f32>,
  @location(5) at: vec3<f32>,
  // Where the corner stands in the sun's depth picture, and its own depth from the sun.
  @location(6) sun_at: vec3<f32>,
}

@vertex
fn rich_vertex(@location(0) position: vec4<i32>, @location(1) uv: vec2<i32>, @location(2) colour: vec4<f32>) -> RichOut {
  var out: RichOut;
  let at = chunk.place.xyz + vec3<f32>(position.xyz) * chunk.place.w;
  let clip = vec4<f32>(at, 1.0) * scene.world;
  out.position = clip;
  out.uv = vec2<f32>(uv) / 32768.0 * chunk.uv.xy + chunk.uv.zw;
  out.colour = colour.rgb;
  // (the shade's red and green are the two bytes after the position's three numbers, where the PSP's
  // corner has padding; its blue travels as the colour's fourth byte)
  out.shade = vec3<f32>(f32(position.w & 255) / 255.0, f32((position.w >> 8u) & 255) / 255.0, colour.a);
  out.clear = clear_at(clip.w);
  out.ground = vec2<f32>(at.x * scene.shadow_u.x + at.z * scene.shadow_u.y + scene.shadow_u.z, at.x * scene.shadow_v.x + at.z * scene.shadow_v.y + scene.shadow_v.z);
  out.at = at;
  let place = vec4<f32>(at, 1.0);
  out.sun_at = vec3<f32>(dot(scene.sun_u, place), dot(scene.sun_v, place), dot(scene.sun_w, place));
  return out;
}

// The sun's depth at a place of its picture, 0…1, mixed between the four texels about it; a place
// outside the picture takes its edge's.
fn sun_reached(uv: vec2<f32>) -> f32 {
  let size = vec2<i32>(textureDimensions(sun_depths));
  let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(size) - 0.5;
  let whole = floor(p);
  let part = p - whole;
  let i = vec2<i32>(whole);
  let last = size - vec2<i32>(1);
  let a = f32(textureLoad(sun_depths, clamp(i, vec2<i32>(0), last), 0).r);
  let b = f32(textureLoad(sun_depths, clamp(i + vec2<i32>(1, 0), vec2<i32>(0), last), 0).r);
  let c = f32(textureLoad(sun_depths, clamp(i + vec2<i32>(0, 1), vec2<i32>(0), last), 0).r);
  let d = f32(textureLoad(sun_depths, clamp(i + vec2<i32>(1, 1), vec2<i32>(0), last), 0).r);
  return mix(mix(a, b, part.x), mix(c, d, part.x), part.y) / 65535.0;
}

@fragment
fn rich_fragment(in: RichOut) -> @location(0) vec4<f32> {
  let texel = textureSample(picture, texels, in.uv);
  let shadow = textureSample(ground_shadow, ground_texels, in.ground).rgb;
  if (chunk.kind.x > 0.5 && texel.a < 0.5) {
    discard;
  }
  var light = in.colour;
  if (chunk.kind.z > 0.5 && scene.sun_edge.w > 0.5) {
    // Lit while the fragment's depth from the sun is no more than the picture's there.
    let sunned = clamp((sun_reached(in.sun_at.xy) + scene.sun_edge.x - in.sun_at.z) * scene.sun_edge.y, 0.0, 1.0);
    light = mix(in.shade, in.colour, sunned);
  }
  var lit = texel.rgb * light * 2.0;
  if (chunk.kind.y > 0.5 && scene.sun_edge.z > 0.5) {
    // (a texel of one half is all of the sun)
    lit *= min(shadow * 2.0, vec3<f32>(1.0));
  }
  if (scene.flash.w > 0.0) {
    let to = in.at - scene.flash.xyz;
    lit += texel.rgb * scene.flash_colour.rgb * (scene.flash.w / (1.0 + dot(to, to)));
  }
  return vec4<f32>(mix(scene.haze_colour.rgb, min(lit, vec3<f32>(1.0)), in.clear), 1.0);
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
