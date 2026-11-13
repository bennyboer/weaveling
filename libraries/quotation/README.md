# quotation

A book's quotation style. This library holds:

- the five styles, by name,
- the style a language uses by default, when no style was chosen,
- the marks each style uses to open and close what is said.

The browser types these marks, and the server rewrites them when the style changes. So both read the rule from here, instead of each keeping its own copy. [`words`](../words) does the same for the rule of what a word is.

A project's style is read from its name, or comes from its language. The style then gives the marks:

```rust
use quotation::QuotationMarks;

let chosen = QuotationMarks::new("low-high")?;
assert_eq!(chosen.opening(), "„");
assert_eq!(chosen.closing(), "“");

let brought = QuotationMarks::of_language(Some("de-AT"));
assert_eq!(brought, QuotationMarks::PointingIn);
assert_eq!(brought.shown(), "»…«");
```
