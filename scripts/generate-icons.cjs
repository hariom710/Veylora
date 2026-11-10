// Generate simple geometric icons for Veylora.
// Creates PNG icons and an ICO file with multiple sizes.
// Design: A simple vault/shield motif in dark-mode palette.

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

// Dark slate color palette matching the app
const BG_COLOR = [30, 41, 59];      // slate-800
const PRIMARY_COLOR = [99, 102, 241]; // indigo-500
const LOCK_COLOR = [203, 213, 225];  // slate-200

function writePNG(filename, width, height, drawFn) {
    const bytesPerRow = width * 3 + 1; // +1 for filter byte
    const rawData = Buffer.alloc(bytesPerRow * height);
    
    for (let y = 0; y < height; y++) {
        const rowOffset = y * bytesPerRow;
        rawData[rowOffset] = 0; // filter: none
        for (let x = 0; x < width; x++) {
            const pixelOffset = rowOffset + 1 + x * 3;
            const [r, g, b] = drawFn(x, y, width, height);
            rawData[pixelOffset] = r;
            rawData[pixelOffset + 1] = g;
            rawData[pixelOffset + 2] = b;
        }
    }

    const compressed = zlib.deflateRawSync(rawData, { level: 9 });

    function chunk(type, data) {
        const len = Buffer.alloc(4);
        len.writeUInt32BE(data.length, 0);
        const typeBuf = Buffer.from(type, 'ascii');
        const crcInput = Buffer.concat([typeBuf, data]);
        const crc = require('crc-32').buf(crcInput);
        const crcBuf = Buffer.alloc(4);
        crcBuf.writeUInt32BE(crc >>> 0, 0);
        return Buffer.concat([len, typeBuf, data, crcBuf]);
    }

    // CRC-32 implementation (simple)
    function crc32(buf) {
        let crc = 0xFFFFFFFF;
        for (let i = 0; i < buf.length; i++) {
            crc ^= buf[i];
            for (let j = 0; j < 8; j++) {
                if (crc & 1) crc = (crc >>> 1) ^ 0xEDB88320;
                else crc = crc >>> 1;
            }
        }
        return (crc ^ 0xFFFFFFFF) >>> 0;
    }

    function chunk2(type, data) {
        const len = Buffer.alloc(4);
        len.writeUInt32BE(data.length, 0);
        const typeBuf = Buffer.from(type, 'ascii');
        const crc = crc32(Buffer.concat([typeBuf, data])) >>> 0;
        const crcBuf = Buffer.alloc(4);
        crcBuf.writeUInt32BE(crc, 0);
        return Buffer.concat([len, typeBuf, data, crcBuf]);
    }

    const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    const ihdrData = Buffer.alloc(13);
    ihdrData.writeUInt32BE(width, 0);
    ihdrData.writeUInt32BE(height, 4);
    ihdrData[8] = 8;  // bit depth
    ihdrData[9] = 2;  // color type: RGB
    ihdrData[10] = 0; // compression
    ihdrData[11] = 0; // filter
    ihdrData[12] = 0; // interlace

    const idat = chunk2('IDAT', compressed);
    const iend = chunk2('IEND', Buffer.alloc(0));

    const png = Buffer.concat([signature, chunk2('IHDR', ihdrData), idat, iend]);
    fs.writeFileSync(filename, png);
    console.log(`Created ${filename} (${width}x${height})`);
}

function drawVaultIcon(x, y, width, height) {
    // Background
    if (x < width / 3 || x > width * 2 / 3 || y < height / 3 || y > height * 2 / 3) {
        return BG_COLOR;
    }

    // Shield shape (vault body)
    const cx = width / 2;
    const cy = height / 2;
    const shieldW = width * 0.4;
    const shieldH = height * 0.4;
    
    const dx = Math.abs(x - cx) / shieldW;
    const dy = Math.abs(y - cy) / shieldH;
    
    // Shield: rounded top, pointed bottom
    if (dx < 0.5 && dy < 0.5) {
        if (y < cy - shieldH * 0.3) {
            // Top rounded part
            const topDist = Math.sqrt(dx * dx + ((y - (cy - shieldH * 0.3)) / (shieldH * 0.3)) ** 2);
            if (topDist < 0.7) return PRIMARY_COLOR;
        } else {
            return PRIMARY_COLOR;
        }
    }

    // Lock shackle (top arc)
    const shackleY = cy - shieldH * 0.35;
    if (y >= shackleY - height * 0.05 && y <= shackleY + height * 0.05) {
        if (x > cx - width * 0.15 && x < cx + width * 0.15) {
            return LOCK_COLOR;
        }
    }

    // Lock body (rectangle below shackle)
    if (y > shackleY && y < cy - shieldH * 0.1) {
        if (x > cx - width * 0.08 && x < cx + width * 0.08) {
            return LOCK_COLOR;
        }
    }

    // Keyhole (circle at bottom of shield)
    if (y > cy + shieldH * 0.1 && y < cy + shieldH * 0.25) {
        const kx = x - cx;
        const ky = y - (cy + shieldH * 0.175);
        if (kx * kx + ky * ky < (width * 0.04) ** 2) {
            return BG_COLOR; // keyhole cutout
        }
    }

    return BG_COLOR;
}

const iconsDir = path.join(__dirname, '..', 'src-tauri', 'icons');
const sizes = [16, 24, 32, 48, 64, 128, 256];

for (const size of sizes) {
    writePNG(path.join(iconsDir, `icon-${size}.png`), size, size, drawVaultIcon);
}

// 512x512 for release artifact thumbnail
writePNG(path.join(iconsDir, 'icon-512.png'), 512, 512, drawVaultIcon);

console.log('\nAll PNG icons generated.');
console.log('Note: Convert PNGs to .ico using an image converter for Windows installer icon.');
console.log('Placeholder icons generated - replace with professional artwork before v1.1.');
