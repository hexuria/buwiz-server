# WebMCP on Buwiz

WebMCP is a **client-side browser API** so an in-browser agent can discover page tools. It is **not** a backend MCP or SSE server.

Spec: [Web Model Context Protocol](https://webmachinelearning.github.io/webmcp/)  
Chrome: [Declarative API](https://developer.chrome.com/docs/ai/webmcp/declarative-api)

## Declarative vs imperative

| Mode | What Buwiz uses | When |
|------|-----------------|------|
Declarative attributes use the spec names `toolname`, `tooldescription`, and `toolparamdescription`. Leptos 0.8 typed HTML does not accept those as ordinary props, so the markup is applied with `leptos::attr::custom::custom_attribute` (same attributes in the rendered HTML).
| **Imperative** | `document.modelContext.registerTool` (fallback `navigator.modelContext`) | Hydrate-only Effect in `WebMcpBootstrap`. SSR never calls it. |

The getter moved from `navigator.modelContext` to `document.modelContext` (tools belong to a document). Chromium still exposes the older name on some builds, so the client probes both.

**Humans stay in the loop.** Auth forms do **not** set `toolautosubmit`. Imperative `execute` fills fields and asks the user to submit. Server functions validate agent input the same as any public API.

`WEBMCP_ENABLED` (Spin variable `webmcp_enabled`, default `true` in local) gates imperative registration. Declarative attributes are inert when the browser has no WebMCP implementation.

## Test locally

1. Chrome / Chromium: `chrome://flags/#enable-webmcp-testing` → Enabled, relaunch.
2. `make dev` and open `http://localhost:3008/login`.
3. Inspector / Model Context Tool Inspector: tools `login` / `register` should appear from the form attributes.
4. Fill via the agent, then click **Sign in** / **Create workspace** yourself.
5. With the flag off or in Firefox/Safari, the page still works; registration is a no-op.

Later tax-profile actions can add the same attributes. Do not add a Spin MCP endpoint for this.
