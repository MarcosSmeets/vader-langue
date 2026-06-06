// Renders icons/icon.png (128x128) for the VS Code marketplace from the "Visor"
// brand mark — no external deps, just zlib + a tiny supersampled rasterizer.
// Mirrors icon.svg: ink tile (20% radius) + saber-red top glow + Visor shield/slit.
//
//   node icons/render-icon.js
const fs = require("fs");
const zlib = require("zlib");
const path = require("path");

const OUT = path.join(__dirname, "icon.png");
const SIZE = 128;     // final px
const SS = 4;         // supersampling factor
const BIG = SIZE * SS;

// palette
const INK = [0x14, 0x16, 0x1b];
const PAPER = [0xf4, 0xf2, 0xec];
const ACCENT = [0xe2, 0x1d, 0x2e];

const SHIELD = [[24, 24], [76, 24], [76, 50], [50, 82], [24, 50]];
const SLIT = [[33, 41], [67, 41], [61, 51], [39, 51]];

function inPoly(x, y, poly) {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i], [xj, yj] = poly[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

// straight RGBA color for a point in 128-space; alpha 0 outside the tile
function sample(px, py) {
  // rounded-rect SDF, centred, half=64, r=25.6
  const lx = Math.abs(px - 64) - (64 - 25.6);
  const ly = Math.abs(py - 64) - (64 - 25.6);
  const qx = Math.max(lx, 0), qy = Math.max(ly, 0);
  const dist = Math.hypot(qx, qy) + Math.min(Math.max(lx, ly), 0) - 25.6;
  if (dist > 0) return [0, 0, 0, 0]; // outside tile -> transparent

  let rgb = INK.slice();

  // top saber glow: radial(120% 90% at 50% 0%), accent@0.12 -> transparent@60%
  const d = Math.hypot((px - 64) / (1.2 * 128), (py - 0) / (0.9 * 128));
  const ga = 0.12 * Math.max(0, Math.min(1, 1 - d / 0.6));
  if (ga > 0) rgb = rgb.map((c, i) => c * (1 - ga) + ACCENT[i] * ga);

  // Visor mark: translate(28.16) scale(0.7168) in tile-space
  const mx = (px - 28.16) / 0.7168;
  const my = (py - 28.16) / 0.7168;
  if (inPoly(mx, my, SLIT)) return [...ACCENT, 255];
  if (inPoly(mx, my, SHIELD)) return [...PAPER, 255];

  return [...rgb, 255];
}

// render big buffer then box-downsample with premultiplied alpha
const px = Buffer.alloc(SIZE * SIZE * 4);
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    let r = 0, g = 0, b = 0, a = 0;
    for (let sy = 0; sy < SS; sy++) {
      for (let sx = 0; sx < SS; sx++) {
        const fx = (x * SS + sx + 0.5) / SS;
        const fy = (y * SS + sy + 0.5) / SS;
        const [cr, cg, cb, ca] = sample(fx, fy);
        const af = ca / 255;
        r += cr * af; g += cg * af; b += cb * af; a += af;
      }
    }
    const n = SS * SS;
    const o = (y * SIZE + x) * 4;
    const alpha = a / n;
    if (a > 0) {
      px[o] = Math.round(r / a);
      px[o + 1] = Math.round(g / a);
      px[o + 2] = Math.round(b / a);
    }
    px[o + 3] = Math.round(alpha * 255);
  }
}

// --- minimal PNG encoder (RGBA, filter 0) ---
const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return (buf) => {
    let c = 0xffffffff;
    for (let i = 0; i < buf.length; i++) c = t[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
})();

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(CRC(td), 0);
  return Buffer.concat([len, td, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8;   // bit depth
ihdr[9] = 6;   // color type RGBA
// 10,11,12 = compression/filter/interlace = 0

const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0; // filter: none
  px.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}

const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

fs.writeFileSync(OUT, png);
console.log(`wrote ${OUT} (${png.length} bytes, ${SIZE}x${SIZE})`);
