import { describe, expect, it } from "vitest";
import { encodeFrame, FrameDecoder, type ProtocolFrame } from "./index.js";

const frame: ProtocolFrame = {
  protocolVersion: 1,
  requestId: "request-1",
  correlationId: "correlation-1",
  sequence: 0,
  kind: "handshake",
  payload: { minVersion: 1, maxVersion: 1 },
};

describe("length-prefixed protocol", () => {
  it("round trips a frame split across arbitrary chunks", () => {
    const encoded = encodeFrame(frame);
    const decoder = new FrameDecoder();
    expect(decoder.push(encoded.subarray(0, 2))).toEqual([]);
    expect(decoder.push(encoded.subarray(2, 9))).toEqual([]);
    expect(decoder.push(encoded.subarray(9))).toEqual([frame]);
  });

  it("decodes multiple frames from one chunk", () => {
    const decoder = new FrameDecoder();
    const result = decoder.push(
      Buffer.concat([encodeFrame(frame), encodeFrame({ ...frame, sequence: 1 })]),
    );
    expect(result.map((entry) => entry.sequence)).toEqual([0, 1]);
  });

  it("rejects oversized declared payloads before buffering them", () => {
    const header = Buffer.alloc(4);
    header.writeUInt32BE(1025, 0);
    expect(() => new FrameDecoder(1024).push(header)).toThrow("maximum is 1024");
  });

  it("enforces a negotiated frame limit", () => {
    const decoder = new FrameDecoder();
    decoder.setMaxBytes(128);
    expect(() => decoder.push(encodeFrame(frame))).toThrow("maximum is 128");
  });

  it("rejects incompatible protocol versions", () => {
    const invalid = Buffer.from(JSON.stringify({ ...frame, protocolVersion: 2 }), "utf8");
    const header = Buffer.alloc(4);
    header.writeUInt32BE(invalid.byteLength, 0);
    expect(() => new FrameDecoder().push(Buffer.concat([header, invalid]))).toThrow(
      "Unsupported protocol version",
    );
  });
});
