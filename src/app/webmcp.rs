//! Client-side WebMCP bootstrap (not a backend MCP/SSE server).
//!
//! Declarative `toolname` / `tooldescription` attributes live on auth forms.
//! Imperative `document.modelContext.registerTool` (fallback:
//! `navigator.modelContext`) runs only inside a hydrate Effect.

#![allow(unused_imports)]
#![allow(clippy::unused_unit)]
#![allow(clippy::unit_arg)]

use crate::app::{browser_load, webmcp_enabled};
use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "hydrate")]
#[wasm_bindgen(inline_js = r#"
export function registerBuwizWebMcpTools() {
  if (window.__buwizWebMcpRegistered) {
    return;
  }
  const ctx =
    (typeof document !== "undefined" && document.modelContext) ||
    (typeof navigator !== "undefined" && navigator.modelContext);
  if (!ctx || typeof ctx.registerTool !== "function") {
    return;
  }
  window.__buwizWebMcpRegistered = true;

  function fillForm(toolName, params) {
    const form =
      document.querySelector('form[toolname="' + toolName + '"]') ||
      document.querySelector("form[data-webmcp-auth]");
    if (!form) {
      return {
        content: [
          {
            type: "text",
            text: "Open /login, /register, or /verify-email/resend so the matching form is on screen.",
          },
        ],
      };
    }
    const entries =
      params && typeof params === "object" ? Object.entries(params) : [];
    for (const [key, value] of entries) {
      const field = form.querySelector('[name="' + key + '"]');
      if (!field) {
        continue;
      }
      field.value = value == null ? "" : String(value);
      field.dispatchEvent(new Event("input", { bubbles: true }));
      field.dispatchEvent(new Event("change", { bubbles: true }));
    }
    form.scrollIntoView({ behavior: "smooth", block: "center" });
    return {
      content: [
        {
          type: "text",
          text: "Fields filled. Review the form and submit yourself — Buwiz never auto-submits auth.",
        },
      ],
    };
  }

  const tools = [
    {
      name: "login",
      description:
        "Fill the Buwiz sign-in form. A human must confirm submit. Never auto-submit credentials.",
      inputSchema: {
        type: "object",
        properties: {
          email: { type: "string", description: "Account email" },
          password: { type: "string", description: "Account password" },
        },
        required: ["email", "password"],
      },
    },
    {
      name: "register",
      description:
        "Fill the Buwiz registration form. A human must confirm submit. Email verification is required before tax profiles.",
      inputSchema: {
        type: "object",
        properties: {
          email: { type: "string", description: "Account email" },
          password: {
            type: "string",
            description: "Password, 15 to 128 characters",
          },
        },
        required: ["email", "password"],
      },
    },
    {
      name: "verify_email",
      description:
        "Fill the email verification token field. A human must confirm submit.",
      inputSchema: {
        type: "object",
        properties: {
          token: { type: "string", description: "One-time verification token" },
        },
        required: ["token"],
      },
    },
    {
      name: "resend_verification",
      description:
        "Fill the resend-verification form. A human must confirm submit. Response is generic whether or not the account exists.",
      inputSchema: {
        type: "object",
        properties: {
          email: { type: "string", description: "Account email" },
        },
        required: ["email"],
      },
    },
  ];

  for (const tool of tools) {
    try {
      ctx.registerTool({
        name: tool.name,
        description: tool.description,
        inputSchema: tool.inputSchema,
        execute: async (params) => fillForm(tool.name, params),
      });
    } catch (_error) {
      // Missing or shifting WebMCP APIs must not break the page.
    }
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = registerBuwizWebMcpTools)]
    fn register_buwiz_webmcp_tools();
}

/// Hydrate-only WebMCP registration. SSR never calls the browser API.
#[island]
pub fn WebMcpBootstrap() -> impl IntoView {
    let enabled = browser_load(webmcp_enabled);
    Effect::new(move |_| {
        if enabled.get() != Some(Ok(true)) {
            return;
        }
        #[cfg(feature = "hydrate")]
        register_buwiz_webmcp_tools();
    });
    view! { <></> }
}
