type CipherRecord = { iv: Uint8Array; ct: ArrayBuffer };

export class SecretStore {
  /** non-extractable AES-256-GCM key lives only in this iframe process */
  private readonly masterKey: Promise<CryptoKey>;
  private readonly vault = new Map<string, CipherRecord>();

  constructor() {
    this.masterKey = crypto.subtle.generateKey(
      { name: "AES-GCM", length: 256 },
      /* extractable */ false,
      ["encrypt", "decrypt"]
    );
  }

  /** Add or replace a secret (plaintext bytes). */
  async add(id: string, plain: Uint8Array): Promise<void> {
    const iv = crypto.getRandomValues(new Uint8Array(12)); // 96-bit GCM nonce
    const key = await this.masterKey;
    const ct = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, plain);
    // Immediately wipe caller’s buffer if you don’t need it afterwards
    plain.fill(0);
    this.vault.set(id, { iv, ct });
  }

  /**
   * Borrow the plaintext for the duration of `fn`.
   * The buffer is zeroed as soon as `fn` resolves, even on throw.
   */
  async withSecret<R>(
    id: string,
    fn: (plain: Uint8Array) => R | Promise<R>
  ): Promise<R | undefined> {
    const rec = this.vault.get(id);
    if (!rec) return undefined;

    const key = await this.masterKey;
    const plainBuf = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: rec.iv },
      key,
      rec.ct
    );

    const plain = new Uint8Array(plainBuf);
    try {
      return await fn(plain); // ← user code can read bytes
    } finally {
      plain.fill(0); // ← zero-wipe
    }
  }

  /** Delete a secret; ciphertext is GC-eligible immediately. */
  remove(id: string): boolean {
    return this.vault.delete(id);
  }
}
