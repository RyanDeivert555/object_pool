# object_pool

A fixed-capacity, allocation-free object pool for Rust, built on const generics and interior mutability.

## Features

- **Zero heap allocation after creation**: backed by a fixed-size array (`ObjectPool<T, N>`)
- **Interior mutability**: no `&mut` needed to take or release objects from the pool
- **Automatic release**: objects return to the pool when their handle is dropped (or manually via `release`)
- **Custom reset logic**: a callback runs on every object as it's returned to the pool
- **Three ways to take an object**: `take`, `take_with_value`, and `lazy_take`, depending on how you need to construct it

## Usage

Not published on crates.io yet. Add it as a git dependency:

```toml
[dependencies]
object_pool = { git = "https://github.com/RyanDeivert555/object_pool" }
```

## Example

```rust
use object_pool::ObjectPool;

// A pool of 10 Vec<i32> buffers, cleared each time they're returned to the pool
let pool: ObjectPool<Vec<i32>, 10> = ObjectPool::new(Vec::clear);

let mut handle = pool.take().unwrap();
handle.push(1);
handle.push(2);
assert_eq!(handle.len(), 2);

// Dropping the handle resets it (via Vec::clear) and returns the slot to the pool
// Or use `pool.release(handle);`
drop(handle);

let handle = pool.take().unwrap();
assert_eq!(handle.len(), 0);
```

## API
- `ObjectPool::new(reset: fn(&mut T)) -> Self`
  Creates a pool of `N` default-constructed values of `T`. `reset` runs on a value each time it's returned to the pool.

- `take(&self) -> Result<Handle<'_, T, N>, NoFreeSlotsError>`
  Reserves a default-constructed slot.

- `take_with_value(&self, value: T) -> Result<Handle<'_, T, N>, NoFreeSlotsError>`
  Reserves a slot and assigns it a value immediately.

- `lazy_take(&self, f: impl FnOnce() -> T) -> Result<Handle<'_, T, N>, NoFreeSlotsError>`
  Like `take_with_value`, but `f` only runs if a slot is actually free. Useful when constructing the value is expensive.

- `release(&self, handle: Handle<'_, T, N>)`
  Explicitly returns a handle to the pool. Functionally the same as dropping it, but asserts the handle belongs to this pool.

`Handle<'pool, T, N>` derefs to `&T` / `&mut T` and automatically returns its slot to the pool (running the `reset` callback) when dropped.

## Why use this?

Fixed-size pools like this are common in game engines and other real-time or performance-sensitive systems, where you want to reuse a bounded set of objects — buffers, entities, particles — without hitting the heap allocator on every request. That avoids both allocation overhead and fragmentation for objects whose max count you know ahead of time.

## Testing

```
cargo test
cargo miri test
```

## License

MIT: see [LICENSE](LICENSE).
