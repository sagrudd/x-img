#!/usr/bin/env node
// SPDX-License-Identifier: MPL-2.0
// Permanent signed-add-on acceptance in an isolated, disposable Firefox profile.

import assert from "node:assert/strict";
import childProcess from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";

const FIREFOX = process.env.FIREFOX || "/Applications/Firefox.app/Contents/MacOS/firefox";
const xpi = path.resolve(process.argv[2] || "");
assert.ok(fs.statSync(xpi).isFile(), "signed XPI path must be a regular file");
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "pinakotheke-signed-install-"));
const profile = path.join(temporary, "profile");
fs.mkdirSync(profile, { mode: 0o700 });

const remotePort = await new Promise((resolve, reject) => {
  const probe = net.createServer();
  probe.once("error", reject);
  probe.listen(0, "127.0.0.1", () => {
    const port = probe.address().port;
    probe.close(error => error ? reject(error) : resolve(port));
  });
});
const firefox = childProcess.spawn(FIREFOX, ["--headless", "--no-remote", "--profile", profile,
  "--remote-debugging-port", String(remotePort), "about:blank"], { stdio: "ignore" });

let socket;
let nextId = 1;
const pending = new Map();
function command(method, params) {
  return new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function connect() {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    try {
      socket = await new Promise((resolve, reject) => {
        const candidate = new WebSocket(`ws://127.0.0.1:${remotePort}/session`);
        candidate.onopen = () => resolve(candidate);
        candidate.onerror = () => reject(new Error("Firefox BiDi unavailable"));
      });
      socket.onmessage = event => {
        const message = JSON.parse(event.data);
        const waiter = pending.get(message.id);
        if (!waiter) return;
        pending.delete(message.id);
        if (message.type === "success") waiter.resolve(message.result);
        else waiter.reject(new Error(message.message));
      };
      return;
    } catch (_) {
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
  throw new Error("Firefox BiDi did not start");
}

try {
  await connect();
  await command("session.new", { capabilities: { alwaysMatch: { acceptInsecureCerts: false } } });
  const installed = await command("webExtension.install", {
    extensionData: { type: "base64", value: fs.readFileSync(xpi).toString("base64") },
    "moz:permanent": true,
  });
  assert.equal(installed.extension, "x-img@example.invalid");
  await command("session.end", {});
  console.log("Permanent Firefox installation passed: Mozilla signature accepted and stable identity installed");
} finally {
  if (socket?.readyState === WebSocket.OPEN) socket.close();
  if (firefox.exitCode === null) {
    firefox.kill("SIGTERM");
    await Promise.race([
      new Promise(resolve => firefox.once("exit", resolve)),
      new Promise(resolve => setTimeout(resolve, 5_000)),
    ]);
  }
  fs.rmSync(temporary, { recursive: true, force: true });
}
