import {
  DEFAULT_MAX_FRAME_BYTES,
  PROTOCOL_VERSION,
  type HandshakeRequest,
  type HandshakeResponse,
  type ProtocolFrame,
} from "@vea/protocol";

export const AGENT_SERVICE_BUILD_ID = "vea-agent-service-dev";

function response<T>(request: ProtocolFrame, kind: string, payload: T): ProtocolFrame<T> {
  return {
    protocolVersion: PROTOCOL_VERSION,
    requestId: request.requestId,
    correlationId: request.correlationId,
    sequence: request.sequence,
    kind,
    payload,
  };
}

export class AgentServiceSession {
  #handshaken = false;
  #maxFrameBytes = DEFAULT_MAX_FRAME_BYTES;

  get maxFrameBytes(): number {
    return this.#maxFrameBytes;
  }

  handle(frame: ProtocolFrame): ProtocolFrame {
    if (!this.#handshaken && frame.kind !== "handshake") {
      return response(frame, "error", { code: "HANDSHAKE_REQUIRED" });
    }

    if (frame.kind === "handshake") {
      if (this.#handshaken) return response(frame, "error", { code: "ALREADY_HANDSHAKEN" });
      const request = frame.payload as Partial<HandshakeRequest>;
      const requestedMaxFrameBytes = request.maxFrameBytes;
      if (
        typeof request.minVersion !== "number" ||
        typeof request.maxVersion !== "number" ||
        PROTOCOL_VERSION < request.minVersion ||
        PROTOCOL_VERSION > request.maxVersion ||
        typeof request.buildId !== "string" ||
        request.buildId.length === 0 ||
        typeof request.instanceNonce !== "string" ||
        request.instanceNonce.length === 0 ||
        typeof requestedMaxFrameBytes !== "number" ||
        !Number.isSafeInteger(requestedMaxFrameBytes) ||
        requestedMaxFrameBytes < 1024
      ) {
        return response(frame, "error", { code: "INCOMPATIBLE_PROTOCOL", supportedVersion: 1 });
      }

      this.#handshaken = true;
      this.#maxFrameBytes = Math.min(requestedMaxFrameBytes, DEFAULT_MAX_FRAME_BYTES);
      const payload: HandshakeResponse = {
        selectedVersion: PROTOCOL_VERSION,
        selectedMaxFrameBytes: this.#maxFrameBytes,
        buildId: AGENT_SERVICE_BUILD_ID,
        instanceNonce: request.instanceNonce,
        features: ["dag-scheduler", "effort-routing", "extension-planes"],
      };
      return response(frame, "handshake.ok", payload);
    }

    if (frame.kind === "ping") {
      return response(frame, "pong", { service: AGENT_SERVICE_BUILD_ID });
    }

    return response(frame, "error", { code: "UNKNOWN_FRAME_KIND", kind: frame.kind });
  }
}
