# quotation

A book's quotation style: the five styles by name, the one a language brings when none was chosen, and the marks each style opens and closes what is said with. The browser types them and the server rewrites them when the style changes, so both read the rule from here rather than each keeping its own, as [`words`](../words) does for the rule for a word.

A project's style is read from its name, or brought by its language, and gives the marks:

```rust
use quotation::QuotationMarks;

let chosen = QuotationMarks::new("low-high")?;
assert_eq!(chosen.opening(), "„");
assert_eq!(chosen.closing(), "“");

let brought = QuotationMarks::of_language(Some("de-AT"));
assert_eq!(brought, QuotationMarks::PointingIn);
assert_eq!(brought.shown(), "»…«");
```
