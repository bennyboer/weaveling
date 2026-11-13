# units

How sizes are written in places where people type them. The settings are the first such place.

**`bytes` reads a size: a number and its unit.**

- The units are `B`, `KB`, `MB`, `GB` and `TB`. Upper or lower case both work, with or without a space before the unit.
- A kilobyte is 1024 bytes. So `KiB`, `MiB`, `GiB` and `TiB` are read as the same units.
- A fraction is written after a decimal point. It is counted exactly, without floating point. For example, `1.5GB` is 1536 megabytes. A leftover part of a byte is dropped.

Some inputs are refused:

- A number without a unit is refused with `UnreadableSize::NoUnit`. It is not taken as bytes. Someone who writes `20` for an upload limit means megabytes, and would otherwise refuse every image.
- A comma is refused with `Comma`. A comma could separate thousands as well as mark a fraction.
- An unknown unit is refused with `UnknownUnit`, which names the unit.
- A size larger than a `u64` can hold is refused with `TooLarge`.

Zero is a valid size. The code that reads a size decides whether zero is allowed there.

```rust
use units::UnreadableSize;

assert_eq!(units::bytes("1GB"), Ok(1024 * 1024 * 1024));
assert_eq!(units::bytes("4096 kb"), Ok(4 * 1024 * 1024));
assert_eq!(units::bytes("1.5gb"), Ok(1536 * 1024 * 1024));
assert_eq!(units::bytes("20"), Err(UnreadableSize::NoUnit));
```
