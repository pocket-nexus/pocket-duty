// One file of the DutyIR: a JSON header and the typed arrays it names, each on a 4-byte boundary.
//
//   u32 magic 'DIR1' · u32 header bytes · header JSON · arrays
//
// The header is `{ ...meta, arrays: { name: { type, offset, length } } }`; `offset` counts from the
// start of the array area and `length` is in elements.
const TYPES = new Map([
  [Float32Array, 'f32'], [Uint32Array, 'u32'], [Uint16Array, 'u16'], [Uint8Array, 'u8'], [Int8Array, 'i8'], [Int16Array, 'i16'], [Int32Array, 'i32'],
]);

export class Bundle {
  constructor(meta = {}) {
    this.meta = meta;
    this.arrays = {};
    this.parts = [];
    this.bytes = 0;
  }

  /** Adds an array and returns the name it was given. */
  add(name, array) {
    const type = TYPES.get(array.constructor);
    if (!type) throw new Error(`bundle: ${name} is not a typed array the IR knows`);
    if (this.arrays[name]) throw new Error(`bundle: ${name} twice`);
    this.arrays[name] = { type, offset: this.bytes, length: array.length };
    const bytes = new Uint8Array(array.buffer, array.byteOffset, array.byteLength);
    this.parts.push(bytes);
    const pad = (4 - (bytes.length & 3)) & 3;
    if (pad) this.parts.push(new Uint8Array(pad));
    this.bytes += bytes.length + pad;
    return name;
  }

  blob() {
    let json = new TextEncoder().encode(JSON.stringify({ ...this.meta, arrays: this.arrays }));
    const pad = (4 - (json.length & 3)) & 3;
    if (pad) json = new Uint8Array([...json, ...new Array(pad).fill(32)]);
    const head = new Uint32Array([0x31524944, json.length]);
    return new Blob([head, json, ...this.parts]);
  }
}
