// Packs dist/chrome and dist/firefox into store-ready zips:
// dist/havenkeys-<browser>-<version>.zip, manifest.json at the zip root.
// Run after build.mjs. No zip dependency or `zip` binary: the container is
// written here with node:zlib, so it works the same on Linux, macOS and
// Windows. Entries are sorted and timestamps fixed, so the same build gives
// byte-identical zips.

import { readFile, readdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, relative, sep } from "node:path";
import { crc32, deflateRawSync } from "node:zlib";

const root = dirname(fileURLToPath(import.meta.url));
const dist = join(root, "dist");

const pkgVersion = JSON.parse(await readFile(join(root, "package.json"), "utf8")).version;

async function listFiles(dir) {
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await listFiles(p)));
    else if (entry.isFile()) out.push(p);
  }
  return out;
}

// 1980-01-01 00:00, the earliest DOS date: fixed for reproducible output.
const DOS_TIME = 0;
const DOS_DATE = (0 << 9) | (1 << 5) | 1;

function zip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const { name, data } of entries) {
    const nameBuf = Buffer.from(name, "utf8");
    const deflated = deflateRawSync(data, { level: 9 });
    const useDeflate = deflated.length < data.length;
    const body = useDeflate ? deflated : data;
    const crc = crc32(data);

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // version needed
    local.writeUInt16LE(0x0800, 6); // UTF-8 names
    local.writeUInt16LE(useDeflate ? 8 : 0, 8);
    local.writeUInt16LE(DOS_TIME, 10);
    local.writeUInt16LE(DOS_DATE, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    local.writeUInt16LE(0, 28);
    locals.push(local, nameBuf, body);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4); // version made by
    central.writeUInt16LE(20, 6); // version needed
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(useDeflate ? 8 : 0, 10);
    central.writeUInt16LE(DOS_TIME, 12);
    central.writeUInt16LE(DOS_DATE, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(body.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    // extra, comment, disk, internal attrs, external attrs: all zero
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBuf);

    offset += local.length + nameBuf.length + body.length;
  }
  const centralSize = centrals.reduce((n, b) => n + b.length, 0);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralSize, 12);
  end.writeUInt32LE(offset, 16);
  if (offset > 0xffffffff || entries.length > 0xffff) throw new Error("too large for a non-zip64 archive");
  return Buffer.concat([...locals, ...centrals, end]);
}

for (const browser of ["chrome", "firefox"]) {
  const dir = join(dist, browser);
  let manifest;
  try {
    manifest = JSON.parse(await readFile(join(dir, "manifest.json"), "utf8"));
  } catch {
    throw new Error(`dist/${browser}/manifest.json not found: run the build first`);
  }
  if (manifest.version !== pkgVersion) {
    throw new Error(
      `manifest version ${manifest.version} does not match package.json version ${pkgVersion}; ` +
        "bump both (manifest/base.json and package.json) and rebuild",
    );
  }

  const files = (await listFiles(dir)).sort();
  const entries = [];
  for (const file of files) {
    entries.push({ name: relative(dir, file).split(sep).join("/"), data: await readFile(file) });
  }
  const out = join(dist, `havenkeys-${browser}-${manifest.version}.zip`);
  await writeFile(out, zip(entries));
  console.log(`${relative(root, out)} (${entries.length} files)`);
}
