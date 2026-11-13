# words

The few rules about text that more than one feature, or the client and the server alike, must agree on.

**`TextRules` checks typed text against what a field allows.** Examples of fields are a name, a title, or a reason for a lock. Each field's rules say:

- how many characters it may hold,
- whether it may be blank,
- whether it may contain line breaks.

`fitted` trims the text and returns it, or returns why it does not fit: `Unfit::Blank`, `ControlCharacter` or `TooLong { max, actual }`. The features keep their own types (`ProjectName`, `AccountName`, `LockReason`) and only borrow the rule. So the limits stay with the thing they limit, while the checking is written only once. Characters are counted, not bytes, because an author reads a limit as a number of letters.

**`words_in` defines what a word count means.** Anything between spaces that contains a letter or a digit is a word. So a dash or an ellipsis on its own is not a word. The browser counts while an author types, and the server counts what it stores. The two must never show an author different numbers. That is why both call this function.

A project's name borrows the rule, and a scene counts its words:

```rust
use words::{TextRules, Unfit};

const RULES: TextRules = TextRules {
    max_chars: 200,
    may_be_blank: false,
    may_break_lines: false,
};

assert_eq!(RULES.fitted("  The Loom "), Ok("The Loom".to_owned()));
assert_eq!(RULES.fitted("   "), Err(Unfit::Blank));

assert_eq!(words::words_in("She paused — then wove on."), 5);
```

**`opening_of` names a scene that has no title.** The name is the scene's first forty characters, with runs of whitespace collapsed into single spaces. If the text is longer, it is cut off with an ellipsis. The margin of the writing view and the outline show the same opening, because both take it from here.
