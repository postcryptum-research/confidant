// The shared HTTP helper serve(): a malformed request target or a malformed body is a client error
// (400), never an uncaught throw that stops the process and never a 500 that echoes parser internals.
import { test } from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import { serve } from "../lib.mjs";

/** One raw request over a fresh socket; returns the status line and body, or "" if the socket closed
 *  with no response (what a crashed/aborted handler looks like). */
function raw(port, bytes) {
  return new Promise((resolve) => {
    const s = net.connect(port, "127.0.0.1", () => s.write(bytes));
    let d = "";
    s.on("data", (c) => (d += c));
    s.setTimeout(3000, () => { s.destroy(); resolve(d); });
    s.on("end", () => resolve(d));
    s.on("error", () => resolve(d));
  });
}
const status = (resp) => Number((/^HTTP\/1\.[01] (\d{3})/.exec(resp) ?? [])[1] ?? 0);
const get = (p) => `GET ${p} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n`;
const post = (p, b) => `POST ${p} HTTP/1.1\r\nHost: x\r\nContent-Length: ${Buffer.byteLength(b)}\r\nConnection: close\r\n\r\n${b}`;

test("serve survives malformed targets and bodies and answers 4xx, never dropping the connection", async () => {
  let handled = 0;
  const server = await serve(0, { "GET /ok": async () => ({ ok: true }), "POST /echo": async (b) => { handled++; return { got: typeof b, keys: Object.keys(b) }; } });
  const port = server.address().port;
  try {
    assert.equal(status(await raw(port, get("/ok"))), 200, "a good request still works");

    // Malformed request targets that make the URL parser throw: each must answer 400, not close.
    for (const p of ["//", "///", "/\\", "/\\\\", "http://[", "//[", "http://"]) {
      const r = await raw(port, get(p));
      assert.equal(status(r), 400, `GET ${JSON.stringify(p)} should be 400, got ${JSON.stringify(r.slice(0, 40))}`);
    }
    // And the server is still answering after all of them (it did not crash).
    assert.equal(status(await raw(port, get("/ok"))), 200, "server still up after malformed targets");

    // Malformed bodies: not-JSON, and JSON that is not an object. Both 400, no 500, no parser prose.
    for (const b of ["{{{", "not json", '{"a":', "null", "[]", "123", '"x"', "true"]) {
      const r = await raw(port, post("/echo", b));
      assert.equal(status(r), 400, `POST body ${JSON.stringify(b)} should be 400, got ${status(r)}`);
      assert.doesNotMatch(r, /position \d+|Unexpected|destructure|SyntaxError/, `body ${JSON.stringify(b)} must not leak parser internals`);
    }
    assert.equal(handled, 0, "no malformed body reached the handler");

    // A well-formed object body reaches the handler as before.
    assert.equal(status(await raw(port, post("/echo", '{"record":1}'))), 200);
    assert.equal(handled, 1);
  } finally {
    server.close();
  }
});
