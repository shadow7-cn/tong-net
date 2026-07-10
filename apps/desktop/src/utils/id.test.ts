import { describe, expect, it } from "vitest";
import { createId } from "./id";

describe("createId", () => {
  it("works when randomUUID is unavailable on an HTTP mobile browser", () => {
    let value = 0;
    const legacyCrypto = {
      getRandomValues: <T extends ArrayBufferView | null>(array: T) => {
        const bytes = array as Uint8Array;
        bytes.forEach((_, index) => { bytes[index] = (value++ + index * 17) % 256; });
        return array;
      },
    } as Pick<Crypto, "getRandomValues">;

    expect(createId(legacyCrypto)).toMatch(/^[0-9a-f-]{36}$/);
  });
});
