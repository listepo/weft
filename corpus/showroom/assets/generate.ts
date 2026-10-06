// Builds the three files of the showroom corpus screen from one octahedron: a glTF binary for
// `<model-viewer>`, a USDZ for RealityKit and a still image for the fallback. The shape is
// authored here, so the files carry no third-party licence: they are Apache-2.0 like the repository.
// Run `node generate.ts` in this folder to rewrite them; a test compares the first two byte for byte.
import { writeFileSync } from "node:fs";
import { deflateSync } from "node:zlib";

type Vec = [number, number, number];

const TOP: Vec = [0, 1, 0];
const BOTTOM: Vec = [0, -1, 0];
const RING: Vec[] = [
  [0.8, 0, 0],
  [0, 0, 0.8],
  [-0.8, 0, 0],
  [0, 0, -0.8],
];
const COLOR: Vec = [0.1, 0.6, 0.6];

// Counter-clockwise seen from outside.
const FACES: [Vec, Vec, Vec][] = RING.flatMap((a, i) => {
  const b = RING[(i + 1) % RING.length] as Vec;
  return [
    [TOP, b, a],
    [BOTTOM, a, b],
  ] as [Vec, Vec, Vec][];
});

const sub = (a: Vec, b: Vec): Vec => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a: Vec, b: Vec): Vec => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const normal = ([a, b, c]: [Vec, Vec, Vec]): Vec => {
  const n = cross(sub(b, a), sub(c, a));
  const length = Math.hypot(...n);
  return [n[0] / length, n[1] / length, n[2] / length];
};

export function glb(): Buffer {
  const positions = Buffer.alloc(FACES.length * 3 * 12);
  const normals = Buffer.alloc(positions.length);
  const indices = Buffer.alloc(FACES.length * 3 * 2);
  FACES.forEach((face, f) => {
    const n = normal(face);
    face.forEach((p, v) => {
      const at = (f * 3 + v) * 12;
      p.forEach((x, k) => positions.writeFloatLE(x, at + 4 * k));
      n.forEach((x, k) => normals.writeFloatLE(x, at + 4 * k));
      indices.writeUInt16LE(f * 3 + v, (f * 3 + v) * 2);
    });
  });
  const binary = Buffer.concat([positions, normals, indices]);
  const json = {
    asset: { version: "2.0", generator: "weft corpus/showroom/assets/generate.ts" },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ mesh: 0, name: "Gem" }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0, NORMAL: 1 }, indices: 2, material: 0 }] }],
    materials: [
      {
        name: "Teal",
        pbrMetallicRoughness: { baseColorFactor: [...COLOR, 1], metallicFactor: 0.2, roughnessFactor: 0.4 },
      },
    ],
    accessors: [
      { bufferView: 0, componentType: 5126, count: FACES.length * 3, type: "VEC3", min: [-0.8, -1, -0.8], max: [0.8, 1, 0.8] },
      { bufferView: 1, componentType: 5126, count: FACES.length * 3, type: "VEC3" },
      { bufferView: 2, componentType: 5123, count: FACES.length * 3, type: "SCALAR" },
    ],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: positions.length, target: 34962 },
      { buffer: 0, byteOffset: positions.length, byteLength: normals.length, target: 34962 },
      { buffer: 0, byteOffset: positions.length * 2, byteLength: indices.length, target: 34963 },
    ],
    buffers: [{ byteLength: binary.length }],
  };
  const pad = (data: Buffer, byte: number) =>
    Buffer.concat([data, Buffer.alloc((4 - (data.length % 4)) % 4, byte)]);
  const jsonChunk = pad(Buffer.from(JSON.stringify(json)), 0x20);
  const binChunk = pad(binary, 0);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + 8 + jsonChunk.length + 8 + binChunk.length, 8);
  const chunk = (type: string, data: Buffer) => {
    const head = Buffer.alloc(8);
    head.writeUInt32LE(data.length, 0);
    head.write(type, 4, "ascii");
    return Buffer.concat([head, data]);
  };
  return Buffer.concat([header, chunk("JSON", jsonChunk), chunk("BIN\0", binChunk)]);
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(data: Buffer): number {
  let c = 0xffffffff;
  for (const byte of data) c = (CRC_TABLE[(c ^ byte) & 255] as number) ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function usda(): string {
  const points = FACES.flatMap((f) => f).map((p) => `(${p.join(", ")})`);
  return [
    "#usda 1.0",
    "(",
    '    defaultPrim = "Gem"',
    "    metersPerUnit = 1",
    '    upAxis = "Y"',
    ")",
    "",
    'def Xform "Gem"',
    "{",
    '    def Mesh "Mesh"',
    "    {",
    `        int[] faceVertexCounts = [${FACES.map(() => 3).join(", ")}]`,
    `        int[] faceVertexIndices = [${points.map((_, i) => i).join(", ")}]`,
    `        point3f[] points = [${points.join(", ")}]`,
    '        uniform token subdivisionScheme = "none"',
    `        color3f[] primvars:displayColor = [(${COLOR.join(", ")})]`,
    "    }",
    "}",
    "",
  ].join("\n");
}

// A USDZ is an uncompressed zip whose file data starts on a 64-byte boundary.
export function usdz(): Buffer {
  const name = Buffer.from("gem.usda");
  const data = Buffer.from(usda());
  const crc = crc32(data);
  const fixed = 30 + name.length;
  const extra = Buffer.alloc((64 - (fixed % 64)) % 64);
  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(data.length, 18);
  local.writeUInt32LE(data.length, 22);
  local.writeUInt16LE(name.length, 26);
  local.writeUInt16LE(extra.length, 28);
  const entry = Buffer.concat([local, name, extra, data]);
  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(data.length, 20);
  central.writeUInt32LE(data.length, 24);
  central.writeUInt16LE(name.length, 28);
  const directory = Buffer.concat([central, name]);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(1, 8);
  end.writeUInt16LE(1, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(entry.length, 16);
  return Buffer.concat([entry, directory, end]);
}

// The still: the gem seen from a fixed camera, flat shaded and anti-aliased by 3x3 sampling.
export function png(width = 320, height = 240): Buffer {
  const SAMPLES = 3;
  const w = width * SAMPLES;
  const h = height * SAMPLES;
  const yaw = 0.6;
  const pitch = 0.35;
  const turn = (p: Vec): Vec => {
    const [x, y, z] = p;
    const x1 = x * Math.cos(yaw) + z * Math.sin(yaw);
    const z1 = -x * Math.sin(yaw) + z * Math.cos(yaw);
    return [x1, y * Math.cos(pitch) - z1 * Math.sin(pitch), y * Math.sin(pitch) + z1 * Math.cos(pitch)];
  };
  const scale = h * 0.4;
  const project = (p: Vec): [number, number, number] => [w / 2 + p[0] * scale, h / 2 - p[1] * scale, p[2]];
  const light: Vec = [-0.4, 0.7, 0.6];
  const lightLength = Math.hypot(...light);
  const background: Vec = [0.93, 0.95, 0.97];
  const pixels = Array.from({ length: w * h }, () => ({ z: Infinity, c: background }));
  for (const face of FACES) {
    const n = turn(normal(face));
    // Seen from the camera on +z: a face turned away is hidden.
    if (n[2] <= 0) continue;
    const shade = 0.35 + 0.65 * Math.max(0, (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) / lightLength);
    const color = COLOR.map((c) => Math.min(1, c * shade * 1.2)) as Vec;
    const [a, b, c] = face.map((p) => project(turn(p))) as [number[], number[], number[]];
    const area = (b[0]! - a[0]!) * (c[1]! - a[1]!) - (b[1]! - a[1]!) * (c[0]! - a[0]!);
    const minX = Math.max(0, Math.floor(Math.min(a[0]!, b[0]!, c[0]!)));
    const maxX = Math.min(w - 1, Math.ceil(Math.max(a[0]!, b[0]!, c[0]!)));
    const minY = Math.max(0, Math.floor(Math.min(a[1]!, b[1]!, c[1]!)));
    const maxY = Math.min(h - 1, Math.ceil(Math.max(a[1]!, b[1]!, c[1]!)));
    for (let y = minY; y <= maxY; y++) {
      for (let x = minX; x <= maxX; x++) {
        const px = x + 0.5;
        const py = y + 0.5;
        const w0 = ((b[0]! - px) * (c[1]! - py) - (b[1]! - py) * (c[0]! - px)) / area;
        const w1 = ((c[0]! - px) * (a[1]! - py) - (c[1]! - py) * (a[0]! - px)) / area;
        const w2 = 1 - w0 - w1;
        if (w0 < 0 || w1 < 0 || w2 < 0) continue;
        const z = w0 * a[2]! + w1 * b[2]! + w2 * c[2]!;
        const pixel = pixels[y * w + x]!;
        // The camera looks down -z, so the larger z is nearer.
        if (pixel.z === Infinity || z > pixel.z) {
          pixel.z = z;
          pixel.c = color;
        }
      }
    }
  }
  const rows: Buffer[] = [];
  for (let y = 0; y < height; y++) {
    const row = Buffer.alloc(1 + width * 3);
    for (let x = 0; x < width; x++) {
      const sum: Vec = [0, 0, 0];
      for (let sy = 0; sy < SAMPLES; sy++) {
        for (let sx = 0; sx < SAMPLES; sx++) {
          const c = pixels[(y * SAMPLES + sy) * w + x * SAMPLES + sx]!.c;
          sum[0] += c[0];
          sum[1] += c[1];
          sum[2] += c[2];
        }
      }
      for (let k = 0; k < 3; k++) row[1 + x * 3 + k] = Math.round((sum[k]! / (SAMPLES * SAMPLES)) * 255);
    }
    rows.push(row);
  }
  const chunk = (type: string, data: Buffer) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(data.length, 0);
    head.write(type, 4, "ascii");
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
    return Buffer.concat([head, data, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(Buffer.concat(rows), { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

if (process.argv[1] === new URL(import.meta.url).pathname) {
  writeFileSync(new URL("gem.glb", import.meta.url), glb());
  writeFileSync(new URL("gem.usdz", import.meta.url), usdz());
  writeFileSync(new URL("gem.png", import.meta.url), png());
}
