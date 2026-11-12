# units

How sizes are written where people write them, in the settings first.

**`bytes` reads a size: a number and its unit.** `B`, `KB`, `MB`, `GB` and `TB`, in any case and with a space or none; a kilobyte is 1024 bytes, so `KiB`, `MiB`, `GiB` and `TiB` are read as the same. A share is written after a point and counted exactly, without floating point, `1.5GB` being 1536 megabytes, and a part of a byte left over is let go. A number alone is refused, `UnreadableSize::NoUnit`, rather than taken for bytes, since whoever writes `20` for an upload means megabytes and would be refusing every image; so are a comma (`Comma`), which could part thousands as well as a share, a unit not known (`UnknownUnit`, naming it) and more than a `u64` holds (`TooLarge`). Nought is a size; whoever reads one decides whether it may be.

```rust
use units::UnreadableSize;

assert_eq!(units::bytes("1GB"), Ok(1024 * 1024 * 1024));
assert_eq!(units::bytes("4096 kb"), Ok(4 * 1024 * 1024));
assert_eq!(units::bytes("1.5gb"), Ok(1536 * 1024 * 1024));
assert_eq!(units::bytes("20"), Err(UnreadableSize::NoUnit));
```
