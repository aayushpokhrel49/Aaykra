# Aaykra Extensions

This directory contains extensions that ship with the Aaykra repository and are maintained alongside it.

## Structure

Aaykra includes support for a number of languages without requiring an extension. Those languages can be found under [`crates/languages/src`](../crates/languages/src).

Support for other languages is done via extensions. The extensions in this directory use the same [zed_extension_api](https://docs.rs/zed_extension_api/latest/zed_extension_api/) as extensions from the Zed extension registry, which Aaykra can install, for providing [language servers](https://zed.dev/docs/extensions/languages#language-servers), [tree-sitter grammars](https://zed.dev/docs/extensions/languages#grammar) and [tree-sitter queries](https://zed.dev/docs/extensions/languages#tree-sitter-queries).

## Dev Extensions

See the docs for [Developing an Extension Locally](https://zed.dev/docs/extensions/developing-extensions#developing-an-extension-locally) for how to work with one of these extensions. The same steps apply in Aaykra.
