import type { ProtocolFrame } from "@vea/protocol";
import { describe, expect, it } from "vitest";
import { AgentServiceSession } from "./server.js";

function request(kind: string, payload: unknown): ProtocolFrame {
  return {
    protocolVersion: 1,
    requestId: "request-1",
    correlationId: "correlation-1",
    sequence: 0,
    kind,
    payload,
  };
}

describe("agent service protocol", () => {
  it("negotiates protocol v1 and echoes the host nonce", () => {
    const result = new AgentServiceSession().handle(
      request("handshake", {
        minVersion: 1,
        maxVersion: 1,
        buildId: "desktop-dev",
        instanceNonce: "nonce-1",
        maxFrameBytes: 1024,
      }),
    );
    expect(result.kind).toBe("handshake.ok");
    expect(result.payload).toMatchObject({
      selectedVersion: 1,
      selectedMaxFrameBytes: 1024,
      instanceNonce: "nonce-1",
    });
  });

  it("fails closed for incompatible versions", () => {
    const result = new AgentServiceSession().handle(
      request("handshake", {
        minVersion: 2,
        maxVersion: 2,
        buildId: "desktop-dev",
        instanceNonce: "nonce-1",
        maxFrameBytes: 1024,
      }),
    );
    expect(result.kind).toBe("error");
    expect(result.payload).toMatchObject({ code: "INCOMPATIBLE_PROTOCOL" });
  });

  it("requires every handshake field", () => {
    const result = new AgentServiceSession().handle(
      request("handshake", {
        minVersion: 1,
        maxVersion: 1,
        instanceNonce: "nonce-1",
      }),
    );
    expect(result.kind).toBe("error");
    expect(result.payload).toMatchObject({ code: "INCOMPATIBLE_PROTOCOL" });
  });

  it("rejects traffic before the handshake", () => {
    const result = new AgentServiceSession().handle(request("ping", {}));
    expect(result.kind).toBe("error");
    expect(result.payload).toMatchObject({ code: "HANDSHAKE_REQUIRED" });
  });
});
