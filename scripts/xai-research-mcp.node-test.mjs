import assert from "node:assert/strict";
import test from "node:test";

import {
  buildResearchRequest,
  normalizeResponse,
  processRequest,
  TOOL_DEFINITIONS,
} from "./xai-research-mcp.mjs";

test("declares the three read-only research tools", () => {
  assert.deepEqual(
    TOOL_DEFINITIONS.map((tool) => tool.name),
    [
      "xai_research.search_web",
      "xai_research.answer_with_sources",
      "xai_research.summarize_url",
    ],
  );
  assert.ok(TOOL_DEFINITIONS.every((tool) => tool.annotations.readOnlyHint));
  assert.ok(TOOL_DEFINITIONS.every((tool) => tool.annotations.openWorldHint));
});

test("builds a non-persistent web search request", () => {
  const request = buildResearchRequest("xai_research.search_web", {
    query: "latest xAI API changes",
    maxSources: 5,
  });
  assert.equal(request.store, false);
  assert.deepEqual(request.tools, [{ type: "web_search" }]);
  assert.match(request.input[0].content, /latest xAI API changes/);
});

test("rejects local and credential-bearing URLs", () => {
  assert.throws(
    () =>
      buildResearchRequest("xai_research.summarize_url", {
        url: "http://127.0.0.1/private",
      }),
    /public/,
  );
  assert.throws(
    () =>
      buildResearchRequest("xai_research.summarize_url", {
        url: "https://user:secret@example.com/",
      }),
    /credentials/,
  );
});

test("normalizes output text, citations, and exact cost", () => {
  const result = normalizeResponse({
    model: "grok-4.3",
    output_text: "Current answer.",
    citations: ["https://example.com/a", "https://example.com/a", "https://example.com/b"],
    usage: { cost_in_usd_ticks: 25_000_000 },
  });
  assert.deepEqual(result.sources, [
    "https://example.com/a",
    "https://example.com/b",
  ]);
  assert.equal(result.costUsd, 0.0025);
});

test("extracts source URLs from inline markdown citations", () => {
  const result = normalizeResponse({
    output_text:
      "Current answer.[[1]](https://example.com/a) More.[[2]](https://example.com/b)",
  });
  assert.deepEqual(result.sources, [
    "https://example.com/a",
    "https://example.com/b",
  ]);
});

test("serves MCP initialize and tools/list", async () => {
  const initialize = await processRequest({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { protocolVersion: "2025-06-18" },
  });
  assert.equal(initialize.result.serverInfo.name, "agentdeck-xai-research-mcp");

  const list = await processRequest({
    jsonrpc: "2.0",
    id: 2,
    method: "tools/list",
  });
  assert.equal(list.result.tools.length, 3);
});

test("returns structured MCP tool output with a mocked xAI response", async () => {
  const response = await processRequest(
    {
      jsonrpc: "2.0",
      id: 3,
      method: "tools/call",
      params: {
        name: "xai_research.answer_with_sources",
        arguments: { question: "What changed?" },
      },
    },
    {
      apiKey: "test-key",
      audit: false,
      fetchImpl: async () =>
        new Response(
          JSON.stringify({
            model: "grok-4.3",
            output_text: "A sourced answer.",
            citations: ["https://example.com/source"],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
    },
  );
  assert.equal(response.result.isError, false);
  assert.deepEqual(response.result.structuredContent.sources, [
    "https://example.com/source",
  ]);
});
