use crate::error::CoreError;
use rquickjs::{Context, Runtime};

pub struct QuickJsEngine {
    #[allow(dead_code)]
    pub runtime: Runtime,
    pub context: Context,
}

impl Default for QuickJsEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default QuickJsEngine")
    }
}

impl QuickJsEngine {
    pub fn new() -> Result<Self, CoreError> {
        let runtime = Runtime::new().map_err(|e| {
            CoreError::SyntaxError(format!("Failed to create QuickJS runtime: {e}"))
        })?;
        let context = Context::full(&runtime).map_err(|e| {
            CoreError::SyntaxError(format!("Failed to create QuickJS context: {e}"))
        })?;

        // Initialize minimal synthetic browser environment (window, document, console)
        context.with(|ctx| -> Result<(), CoreError> {
            let polyfill = r#"
                globalThis.window = globalThis;
                globalThis.self = globalThis;
                globalThis.document = {
                    createElementNS: function(ns, tag) {
                        return {
                            tagName: tag,
                            attributes: {},
                            children: [],
                            setAttribute: function(k, v) { this.attributes[k] = String(v); },
                            getAttribute: function(k) { return this.attributes[k]; },
                            appendChild: function(c) { this.children.push(c); return c; },
                            style: {},
                        };
                    },
                    createElement: function(tag) { return this.createElementNS("", tag); },
                    createTextNode: function(text) { return { text: text }; },
                    body: { appendChild: function() {} }
                };
            "#;
            let _: () = ctx.eval(polyfill).map_err(|e| {
                CoreError::SyntaxError(format!("Polyfill initialization failed: {e}"))
            })?;
            Ok(())
        })?;

        Ok(Self { runtime, context })
    }

    /// Evaluates JavaScript code in the QuickJS environment and returns the string result.
    pub fn eval(&self, script: &str) -> Result<String, CoreError> {
        self.context.with(|ctx| {
            let result: rquickjs::Value = ctx
                .eval(script)
                .map_err(|e| CoreError::SyntaxError(format!("QuickJS eval failed: {e}")))?;

            if let Some(s) = result.as_string() {
                s.to_string()
                    .map_err(|e| CoreError::SyntaxError(e.to_string()))
            } else {
                Ok(format!("{:?}", result))
            }
        })
    }

    /// Run with QuickJS Context
    pub fn with_context<F, R>(&self, f: F) -> R
    where
        F: FnOnce(rquickjs::Ctx<'_>) -> R,
    {
        self.context.with(f)
    }
}
