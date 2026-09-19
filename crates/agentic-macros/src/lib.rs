//! Proc macros for `agentic`. Users never depend on this crate directly.
//!

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, FnArg, ItemFn, Lit, Meta, Pat, ReturnType, parse_macro_input, punctuated::Punctuated,
    token::Comma,
};

/// Turns an `async fn` into a tool.
///
/// ```ignore
/// /// Get the current weather for a city.
/// #[tool]
/// async fn get_weather(city: String) -> anyhow::Result<String> { ... }
/// ```
///
/// The doc comment becomes the tool description (or pass `#[tool(description = "...")]`).
/// Arguments must implement `serde::Deserialize` and `schemars::JsonSchema`; the return
/// value must implement `serde::Serialize`. The function name becomes a value implementing
/// `agentic::Tool`, so it can be passed straight to `Agent::builder(..).tool(get_weather)`.
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr with Punctuated::<Meta, Comma>::parse_terminated);
    let func = parse_macro_input!(item as ItemFn);
    match expand(attrs, func) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(attrs: Punctuated<Meta, Comma>, func: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig,
            "#[tool] functions must be async",
        ));
    }

    let name = &func.sig.ident;
    let name_str = name.to_string();
    let vis = &func.vis;
    let inner = format_ident!("__agentic_tool_impl_{}", name);
    let args_ty = format_ident!("__AgenticArgs_{}", name);

    let mut description: Option<String> = None;
    for meta in attrs {
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("description") => {
                if let Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = nv.value
                {
                    description = Some(s.value());
                }
            }
            other => {
                return Err(syn::Error::new_spanned(other, "unknown #[tool] option"));
            }
        }
    }
    if description.is_none() {
        let doc: Vec<String> = func
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("doc"))
            .filter_map(|a| match &a.meta {
                Meta::NameValue(nv) => match &nv.value {
                    Expr::Lit(syn::ExprLit {
                        lit: Lit::Str(s), ..
                    }) => Some(s.value().trim().to_owned()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        description = Some(doc.join(" "));
    }
    let description = description.unwrap_or_default();

    let mut field_idents = Vec::new();
    let mut field_tys = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(pt) => {
                let Pat::Ident(pi) = &*pt.pat else {
                    return Err(syn::Error::new_spanned(
                        &pt.pat,
                        "#[tool] arguments must be plain identifiers",
                    ));
                };
                field_idents.push(pi.ident.clone());
                field_tys.push((*pt.ty).clone());
            }
            FnArg::Receiver(r) => {
                return Err(syn::Error::new_spanned(
                    r,
                    "#[tool] functions cannot take self",
                ));
            }
        }
    }

    if !matches!(func.sig.output, ReturnType::Type(..)) {
        return Err(syn::Error::new_spanned(
            &func.sig,
            "#[tool] functions must return a Result",
        ));
    }

    let mut inner_fn = func.clone();
    inner_fn.sig.ident = inner.clone();
    inner_fn.attrs.retain(|a| !a.path().is_ident("doc"));
    inner_fn.vis = syn::Visibility::Inherited;

    Ok(quote! {
        #inner_fn

        #[derive(::agentic::__private::serde::Deserialize, ::agentic::__private::schemars::JsonSchema)]
        #[serde(crate = "::agentic::__private::serde")]
        #[schemars(crate = "::agentic::__private::schemars")]
        #[allow(non_camel_case_types)]
        struct #args_ty {
            #( #field_idents: #field_tys, )*
        }

        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy)]
        #vis struct #name;

        impl ::agentic::Tool for #name {
            fn name(&self) -> &str { #name_str }
            fn description(&self) -> &str { #description }
            fn schema(&self) -> ::agentic::__private::serde_json::Value {
                ::agentic::__private::serde_json::to_value(
                    ::agentic::__private::schemars::schema_for!(#args_ty)
                ).expect("tool schema serializes")
            }
            fn call(
                &self,
                args: ::agentic::__private::serde_json::Value,
            ) -> ::agentic::__private::BoxFuture<'static, ::std::result::Result<::agentic::__private::serde_json::Value, ::agentic::ToolError>> {
                Box::pin(async move {
                    let parsed: #args_ty = ::agentic::__private::serde_json::from_value(args)
                        .map_err(|e| ::agentic::ToolError::InvalidArguments(e.to_string()))?;
                    let out = #inner( #( parsed.#field_idents ),* )
                        .await
                        .map_err(|e| ::agentic::ToolError::Failed(e.to_string()))?;
                    ::agentic::__private::serde_json::to_value(out)
                        .map_err(|e| ::agentic::ToolError::Failed(e.to_string()))
                })
            }
        }
    })
}
