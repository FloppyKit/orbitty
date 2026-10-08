// NIP-42 AUTH (kind 22242) for the Orbitty relay.
// The secret stays in this function's local bytes and is wiped before return.
// Do not log it, do not write it, do not put it on the event.

import { schnorr } from "@noble/curves/secp256k1.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils.js";
import { bech32 } from "@scure/base";

export const KIND_AUTH = 22242;

export type SignedEvent = {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
};

const UNREADABLE = "unreadable nsec";

/** 32-byte secret from a bech32 nsec, or 64 hex characters. */
export function secretFromNsec(text: string): Uint8Array {
  const raw = text.trim();
  if (!raw) throw new Error(UNREADABLE);
  try {
    if (/^[0-9a-fA-F]{64}$/.test(raw)) return hexToBytes(raw.toLowerCase());
    const decoded = bech32.decodeToBytes(raw);
    if (decoded.prefix !== "nsec" || decoded.bytes.length !== 32) throw new Error(UNREADABLE);
    return decoded.bytes;
  } catch (err) {
    if (err instanceof Error && err.message === UNREADABLE) throw err;
    throw new Error(UNREADABLE);
  }
}

/** NIP-01 event id. Compact JSON, same array shape the relay hashes. */
export function nip01Id(
  pubkey: string,
  createdAt: number,
  kind: number,
  tags: readonly (readonly string[])[],
  content: string,
): string {
  const payload = JSON.stringify([0, pubkey, createdAt, kind, tags, content]);
  return bytesToHex(sha256(new TextEncoder().encode(payload)));
}

export function signEvent(
  secret: Uint8Array,
  createdAt: number,
  kind: number,
  tags: string[][],
  content: string,
): SignedEvent {
  const pubkey = bytesToHex(schnorr.getPublicKey(secret));
  const id = nip01Id(pubkey, createdAt, kind, tags, content);
  const sig = bytesToHex(schnorr.sign(hexToBytes(id), secret));
  return { id, pubkey, created_at: createdAt, kind, tags, content, sig };
}

/** Kind 22242. Tags are the relay URL and the challenge the socket just sent. */
export function signAuth(
  nsec: string,
  relayUrl: string,
  challenge: string,
  createdAt = Math.floor(Date.now() / 1000),
): SignedEvent {
  const secret = secretFromNsec(nsec);
  try {
    return signEvent(
      secret,
      createdAt,
      KIND_AUTH,
      [
        ["relay", relayUrl],
        ["challenge", challenge],
      ],
      "",
    );
  } finally {
    secret.fill(0);
  }
}

export function verifySigned(event: SignedEvent): boolean {
  const id = nip01Id(event.pubkey, event.created_at, event.kind, event.tags, event.content);
  if (id !== event.id) return false;
  return schnorr.verify(hexToBytes(event.sig), hexToBytes(event.id), hexToBytes(event.pubkey));
}
