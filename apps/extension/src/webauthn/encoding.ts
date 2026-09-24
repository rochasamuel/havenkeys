// Base64url and BufferSource helpers for the passkey bridge.
//
// Written out rather than using atob/btoa: page.ts runs in the page's own
// world, where the page can replace those globals after we start.

const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

export function toB64Url(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i] as number;
    const b = bytes[i + 1];
    const c = bytes[i + 2];
    out += ALPHABET.charAt(a >> 2) + ALPHABET.charAt(((a & 3) << 4) | ((b ?? 0) >> 4));
    if (b !== undefined) out += ALPHABET.charAt(((b & 15) << 2) | ((c ?? 0) >> 6));
    if (c !== undefined) out += ALPHABET.charAt(c & 63);
  }
  return out;
}

/** Decode unpadded base64url; null for anything else. */
export function fromB64Url(s: string): Uint8Array | null {
  if (s.length % 4 === 1) return null;
  const out = new Uint8Array(Math.floor(s.length / 4) * 3 + ([0, 0, 1, 2][s.length % 4] as number));
  let acc = 0;
  let bits = 0;
  let j = 0;
  for (let i = 0; i < s.length; i++) {
    const v = ALPHABET.indexOf(s.charAt(i));
    if (v < 0) return null;
    acc = ((acc << 6) | v) & 0xffffff;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[j++] = (acc >> bits) & 0xff;
    }
  }
  return out;
}

/** An ArrayBuffer for a base64url string the protocol already validated. */
export function toArrayBuffer(s: string): ArrayBuffer {
  const bytes = fromB64Url(s);
  if (!bytes) throw new TypeError("invalid base64url");
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

/** A copy of an ArrayBuffer or ArrayBufferView's bytes, or null. */
export function bufferSourceBytes(v: unknown): Uint8Array | null {
  try {
    if (v instanceof ArrayBuffer) return new Uint8Array(v.slice(0));
    if (ArrayBuffer.isView(v)) return new Uint8Array(v.buffer.slice(v.byteOffset, v.byteOffset + v.byteLength));
  } catch {
    // Detached buffers throw.
  }
  return null;
}
