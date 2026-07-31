export const PROTOCOL_VERSION = 1;
export const DEFAULT_MAX_FRAME_BYTES = 1024 * 1024;

export interface ProtocolFrame<T = unknown> {
  protocolVersion: 1;
  requestId: string;
  correlationId: string;
  sequence: number;
  kind: string;
  payload: T;
}

export interface HandshakeRequest {
  minVersion: number;
  maxVersion: number;
  buildId: string;
  instanceNonce: string;
  maxFrameBytes: number;
}

export interface HandshakeResponse {
  selectedVersion: 1;
  selectedMaxFrameBytes: number;
  buildId: string;
  instanceNonce: string;
  features: readonly string[];
}

export function encodeFrame(frame: ProtocolFrame, maxBytes = DEFAULT_MAX_FRAME_BYTES): Buffer {
  const payload = Buffer.from(JSON.stringify(frame), "utf8");
  if (payload.byteLength > maxBytes) {
    throw new RangeError(`Protocol frame exceeds ${maxBytes} bytes`);
  }
  const header = Buffer.allocUnsafe(4);
  header.writeUInt32BE(payload.byteLength, 0);
  return Buffer.concat([header, payload]);
}

export class FrameDecoder {
  #maxBytes: number;
  #buffer = Buffer.alloc(0);

  constructor(maxBytes = DEFAULT_MAX_FRAME_BYTES) {
    this.#maxBytes = maxBytes;
  }

  setMaxBytes(maxBytes: number): void {
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1) {
      throw new RangeError("Protocol maximum frame size must be a positive safe integer");
    }
    this.#maxBytes = maxBytes;
  }

  push(chunk: Uint8Array): ProtocolFrame[] {
    this.#buffer = Buffer.concat([this.#buffer, Buffer.from(chunk)]);
    const frames: ProtocolFrame[] = [];

    while (this.#buffer.byteLength >= 4) {
      const size = this.#buffer.readUInt32BE(0);
      if (size > this.#maxBytes) {
        this.#buffer = Buffer.alloc(0);
        throw new RangeError(`Protocol frame declares ${size} bytes; maximum is ${this.#maxBytes}`);
      }
      if (this.#buffer.byteLength < 4 + size) break;
      const body = this.#buffer.subarray(4, 4 + size);
      this.#buffer = this.#buffer.subarray(4 + size);
      const parsed: unknown = JSON.parse(body.toString("utf8"));
      frames.push(assertProtocolFrame(parsed));
    }

    return frames;
  }
}

export function assertProtocolFrame(value: unknown): ProtocolFrame {
  if (!value || typeof value !== "object") throw new TypeError("Protocol frame must be an object");
  const frame = value as Record<string, unknown>;
  if (frame.protocolVersion !== PROTOCOL_VERSION) {
    throw new TypeError(`Unsupported protocol version ${String(frame.protocolVersion)}`);
  }
  if (typeof frame.requestId !== "string" || typeof frame.correlationId !== "string") {
    throw new TypeError("Protocol frame IDs must be strings");
  }
  if (!Number.isSafeInteger(frame.sequence) || (frame.sequence as number) < 0) {
    throw new TypeError("Protocol frame sequence must be a non-negative safe integer");
  }
  if (typeof frame.kind !== "string" || !frame.kind)
    throw new TypeError("Protocol frame kind is required");
  return frame as unknown as ProtocolFrame;
}
