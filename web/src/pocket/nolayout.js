// What the reference's world modules import as their layout when the map is a place of ours
// (maps/<id>/place.js): a street of no length, no alleys, no buildings, no set pieces. The reference's
// ground, gate, perimeter and street dressing are not run for a place (./placeworld.js); its kit, its
// props and its materials are, and they read these names when they are loaded.
export const STREET = { halfWidth: 0, kerb: 0, walkH: 0, zMin: 0, zMax: 0 };
export const ALLEYS = [];
export const BUILDINGS = [];
export const GATE = { z: 0, depth: 0, span: 0, height: 0, outerW: 0, bodyH: 0, xL0: 0, xL1: 0, hL: 0, xR0: 0, xR1: 0, hR: 0, eastProud: 0, xT0: 0, xT1: 0, hT: 0, towerProud: 0 };
export const SET_PIECES = { stalls: [], jerseys: [], sandbagWalls: [], wrecks: [], palms: [], lamps: [], cables: [], laundry: [], hangings: [], rubble: [], tyres: [] };
