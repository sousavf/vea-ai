import { encodeFrame, FrameDecoder } from "@vea/protocol";
import { AgentServiceSession } from "./server.js";

// The handshake is deliberately bounded to the protocol minimum. Only after
// negotiation may either peer send larger frames.
const decoder = new FrameDecoder(1024);
const session = new AgentServiceSession();

process.stdin.on("data", (chunk: Buffer) => {
  try {
    for (const frame of decoder.push(chunk)) {
      const response = session.handle(frame);
      if (response.kind === "handshake.ok") decoder.setMaxBytes(session.maxFrameBytes);
      process.stdout.write(encodeFrame(response, session.maxFrameBytes));
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown protocol error";
    process.stderr.write(`agent-service protocol failure: ${message}\n`);
    process.exitCode = 1;
    process.stdin.destroy();
  }
});

process.stdin.on("error", (error) => {
  process.stderr.write(`agent-service input failure: ${error.message}\n`);
  process.exitCode = 1;
});
