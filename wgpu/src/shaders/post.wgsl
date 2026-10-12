// The frame onto the screen, in the PSP display's own colours: five bits of red, six of green
// and five of blue, with the GE's ordered dither.

struct Post {
  // x: 1 for the 16-bit colours, 0 for the frame as it was drawn.
  look: vec4<f32>,
}
@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var frame: texture_2d<f32>;

// One triangle over the whole screen.
@vertex
fn corner(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
  let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
  return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn present(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
  let pixel = vec2<i32>(at.xy);
  let colour = textureLoad(frame, pixel, 0).rgb;
  if (post.look.x < 0.5) {
    return vec4<f32>(colour, 1.0);
  }
  // What the GE adds to a colour of eight bits before it drops the low ones, by the pixel's
  // place in a square of four (`sceGuSetDither` in psp/src/gfx.rs).
  var matrix = array<f32, 16>(-4.0, 0.0, -3.0, 1.0, 2.0, -2.0, 3.0, -1.0, -3.0, 1.0, -4.0, 0.0, 3.0, -1.0, 2.0, -2.0);
  let nudge = matrix[(pixel.y & 3) * 4 + (pixel.x & 3)];
  let eight = clamp(floor(colour * 255.0 + 0.5) + nudge, vec3<f32>(0.0), vec3<f32>(255.0));
  let kept = floor(eight / vec3<f32>(8.0, 4.0, 8.0));
  // A colour of five or six bits is shown with its high bits again in the low ones.
  let shown = kept * vec3<f32>(8.0, 4.0, 8.0) + floor(kept / vec3<f32>(4.0, 16.0, 4.0));
  return vec4<f32>(shown / 255.0, 1.0);
}
