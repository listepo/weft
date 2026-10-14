//! The accessibility tree a document declares (SPEC §9), as the reference renderer
//! (`@weft/render-react`) exposes it, in the shape of a Playwright aria snapshot. Hosts without a
//! browser show it to agents (`weft_render`), and the renderer's tests check it against the DOM.

mod inst;

#[cfg(test)]
mod tests;
