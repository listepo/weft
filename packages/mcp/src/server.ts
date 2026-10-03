#!/usr/bin/env node
// Entry point: serves the Weft tools over stdio. stdout carries the protocol, so nothing else
// may write to it.
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { createServer } from "./create-server.ts";

if (import.meta.main) {
  await createServer().connect(new StdioServerTransport());
}
