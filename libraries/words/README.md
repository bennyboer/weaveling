# words

The few rules about text that more than one feature, or the client and the server alike, must agree on.

**`TextRules` fits what was typed to what a field allows.** A name, a title, a reason for a lock: each says how many characters it may hold, whether it may be blank and whether it may break lines, and `fitted` trims it and gives it back or says why not, `Unfit::Blank`, `ControlCharacter` or `TooLong { max, actual }`. The features keep their own types (`ProjectName`, `AccountName`, `LockReason`) and only borrow the rule, so the limits stay with the thing they limit while the checking is written once. Characters are counted, not bytes, since a limit an author reads is one of letters.

**`words_in` is what a word count means.** Whatever stands between spaces and holds a letter or a digit is a word, so a dash or an ellipsis standing alone is not. The browser counts as an author types and the server counts what it stores, and the two must never tell an author different numbers, which is why both call this.

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

**`opening_of` is how a scene without a title is named:** its first forty characters with the spaces closed up, cut with an ellipsis. The writing view's margin and the outline show the same opening because they take it from here.
