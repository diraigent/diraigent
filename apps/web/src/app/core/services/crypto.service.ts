import { Injectable, signal } from '@angular/core';

/**
 * Client-side encryption service using the Web Crypto API.
 *
 * Mirrors the server-side `crypto.rs` conventions:
 *   - `enc:v1:<base64(nonce || ciphertext || tag)>` format
 *   - AES-256-GCM with Associated Authenticated Data (AAD)
 *   - HKDF-SHA256 for KEK derivation
 *   - AES-KW (RFC 3394) for DEK wrapping
 *
 * The DEK is held as a non-extractable CryptoKey in memory.
 * Lost on page refresh — must re-derive from passphrase or access token.
 */

const ENC_PREFIX = 'enc:v1:';
const NONCE_LEN = 12;
const DEK_LEN = 32; // AES-256 = 256 bits = 32 bytes
const KEK_INFO = new TextEncoder().encode('diraigent-kek-v1');

@Injectable({ providedIn: 'root' })
export class CryptoService {
  /** Whether a DEK is currently loaded and available for crypto operations. */
  readonly isUnlocked = signal(false);

  private dek: CryptoKey | null = null;

  // ── DEK Management ──

  /** Check if the vault is unlocked (DEK available). */
  hasKey(): boolean {
    return this.dek !== null;
  }

  /** Clear the DEK from memory (lock the vault). */
  lock(): void {
    this.dek = null;
    this.isUnlocked.set(false);
  }

  /**
   * Import a raw DEK from bytes (e.g. for testing or direct key injection).
   * The key is imported as non-extractable for AES-GCM operations.
   */
  async importDek(rawKey: ArrayBuffer): Promise<void> {
    this.dek = await crypto.subtle.importKey(
      'raw',
      rawKey,
      { name: 'AES-GCM', length: 256 },
      false, // non-extractable
      ['encrypt', 'decrypt'],
    );
    this.isUnlocked.set(true);
  }

  // ── Key Derivation ──

  /**
   * Derive a KEK from a secret (access token or passphrase) and a salt.
   *
   * Matches the server-side `derive_kek()`:
   *   HKDF-SHA256(SHA256(secret), base64decode(salt), "diraigent-kek-v1")
   *
   * Returns raw KEK bytes (32 bytes).
   */
  async deriveKek(secret: string, saltB64: string): Promise<ArrayBuffer> {
    // SHA-256 hash of the secret (matches server: Sha256::digest(token))
    const secretBytes = new TextEncoder().encode(secret);
    const secretHash = await crypto.subtle.digest('SHA-256', secretBytes);

    // Import the hash as HKDF key material
    const hkdfKey = await crypto.subtle.importKey(
      'raw',
      secretHash,
      'HKDF',
      false,
      ['deriveBits'],
    );

    // Decode the base64 salt
    const salt = base64ToBytes(saltB64).buffer as ArrayBuffer;

    // HKDF-SHA256 expand with info="diraigent-kek-v1"
    const kekBits = await crypto.subtle.deriveBits(
      { name: 'HKDF', hash: 'SHA-256', salt, info: KEK_INFO },
      hkdfKey,
      DEK_LEN * 8, // 256 bits
    );

    return kekBits;
  }

  // ── Key Wrapping ──

  /**
   * Unwrap a DEK using AES-KW and store it in memory.
   *
   * @param wrappedDekB64 - Base64-encoded AES-KW wrapped DEK
   * @param kekRaw - Raw KEK bytes (32 bytes)
   */
  async unwrapAndStoreDek(wrappedDekB64: string, kekRaw: ArrayBuffer): Promise<void> {
    const kek = await crypto.subtle.importKey(
      'raw',
      kekRaw,
      'AES-KW',
      false,
      ['unwrapKey'],
    );

    const wrappedDek = base64ToBytes(wrappedDekB64).buffer as ArrayBuffer;

    this.dek = await crypto.subtle.unwrapKey(
      'raw',
      wrappedDek,
      kek,
      'AES-KW',
      { name: 'AES-GCM', length: 256 },
      false, // non-extractable
      ['encrypt', 'decrypt'],
    );
    this.isUnlocked.set(true);
  }

  /**
   * Wrap the current DEK with a KEK for storage.
   * Requires a temporary extractable copy of the DEK.
   *
   * @param dekRaw - Raw DEK bytes to wrap
   * @param kekRaw - Raw KEK bytes (32 bytes)
   * @returns Base64-encoded wrapped DEK
   */
  async wrapDek(dekRaw: ArrayBuffer, kekRaw: ArrayBuffer): Promise<string> {
    const kek = await crypto.subtle.importKey(
      'raw',
      kekRaw,
      'AES-KW',
      false,
      ['wrapKey'],
    );

    // Import the DEK as extractable for wrapping
    const dekKey = await crypto.subtle.importKey(
      'raw',
      dekRaw,
      { name: 'AES-GCM', length: 256 },
      true, // extractable (needed for wrapping)
      ['encrypt', 'decrypt'],
    );

    const wrapped = await crypto.subtle.wrapKey('raw', dekKey, kek, 'AES-KW');
    return bytesToBase64(new Uint8Array(wrapped));
  }

  // ── Encrypt / Decrypt ──

  /**
   * Encrypt a string value. Returns `enc:v1:<base64(nonce || ciphertext || tag)>`.
   *
   * @param plaintext - The string to encrypt
   * @param aad - Associated Authenticated Data (e.g. "task.context")
   */
  async encryptStr(plaintext: string, aad: string): Promise<string> {
    if (!this.dek) throw new Error('Vault is locked — no DEK available');

    const ptBytes = new TextEncoder().encode(plaintext);
    const aadBytes = new TextEncoder().encode(aad);

    // Generate random nonce
    const nonce = crypto.getRandomValues(new Uint8Array(NONCE_LEN));

    // AES-256-GCM encrypt (tag is appended to ciphertext by Web Crypto API)
    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: nonce, additionalData: aadBytes, tagLength: 128 },
      this.dek,
      ptBytes,
    );

    // Combine: nonce || ciphertext (includes tag)
    const combined = new Uint8Array(NONCE_LEN + ciphertext.byteLength);
    combined.set(nonce, 0);
    combined.set(new Uint8Array(ciphertext), NONCE_LEN);

    return ENC_PREFIX + bytesToBase64(combined);
  }

  /**
   * Decrypt a value produced by `encryptStr`. Returns the plaintext string.
   * If the value doesn't have the `enc:v1:` prefix, returns it as-is.
   *
   * @param value - The encrypted string (or plaintext passthrough)
   * @param aad - AAD that was used during encryption
   */
  async decryptStr(value: string, aad: string): Promise<string> {
    if (!value.startsWith(ENC_PREFIX)) return value;
    if (!this.dek) throw new Error('Vault is locked — no DEK available');

    const encoded = value.slice(ENC_PREFIX.length);
    const combined = base64ToBytes(encoded);

    const nonce = combined.slice(0, NONCE_LEN);
    const ciphertext = combined.slice(NONCE_LEN);
    const aadBytes = new TextEncoder().encode(aad);

    const plaintext = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: nonce, additionalData: aadBytes, tagLength: 128 },
      this.dek,
      ciphertext,
    );

    return new TextDecoder().decode(plaintext);
  }

  /**
   * Encrypt a JSON value by serializing, encrypting, and returning as a string.
   */
  async encryptJson(value: unknown, aad: string): Promise<string> {
    const json = JSON.stringify(value);
    return this.encryptStr(json, aad);
  }

  /**
   * Decrypt a JSON value. If the value is a string with `enc:v1:` prefix,
   * decrypt and parse. Otherwise return as-is.
   */
  async decryptJson(value: unknown, aad: string): Promise<unknown> {
    if (typeof value === 'string' && value.startsWith(ENC_PREFIX)) {
      const decrypted = await this.decryptStr(value, aad);
      return JSON.parse(decrypted);
    }
    return value;
  }

  // ── Utility ──

  /** Generate a random 32-byte DEK. Returns raw bytes. */
  generateDek(): Uint8Array {
    return crypto.getRandomValues(new Uint8Array(DEK_LEN));
  }

  /** Generate a random 32-byte salt. Returns base64-encoded string. */
  generateSalt(): string {
    const salt = crypto.getRandomValues(new Uint8Array(32));
    return bytesToBase64(salt);
  }

  /** Check if a string value is encrypted. */
  isEncrypted(value: string): boolean {
    return value.startsWith(ENC_PREFIX);
  }
}

// ── Base64 helpers (standard, not URL-safe) ──

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
