# comptr-rs
Smart pointer for [Component Object Model](https://msdn.microsoft.com/en-us/library/windows/desktop/ms680573(v=vs.85).aspx) interfaces.

This crate is designed to be used together with `winapi-rs`. You can use the COM interfaces declared by it or use
`RIDL!` macro from that same crate to declare your COM interfaces.

**Note**: This crate currently requires a nightly version of Rust, since it uses the [Shared](https://doc.rust-lang.org/std/ptr/struct.Shared.html) struct.

## Non-null guarantee
The `ComPtr` type is built around the invariant that the pointer it manages will **never** be null. Since memory in COM is only freed when the last reference is released, and `ComPtr` only releases its reference when the destructor is run, the invariant is maintained.

## Example
The following (not actually real) example shows how you would use the library:

```rust
// Import the crate.
extern crate comptr;
use comptr::ComPtr;

// This function is exported from some DLL.
extern "system" {
    fn CreateInterface(*mut *mut IUnknown);
}

let interface = ComPtr::new({
    let mut ptr = ptr::null_mut();

    unsafe {
        CreateInterface(&mut ptr);
    }

    // The pointer must be non-null or undefined behaviour could occur.
    if ptr == std::ptr::null_mut() {
        panic!("Failed to create COM interface.");
    }

    ptr
});

// Now you can use the interface, with 100% guarantee it is not null.
unsafe {
    interface.CallFunction();
}
```

## Contributing
Issues (i.e. feature requests and bug reports) and pull-requests are welcome!

You can also help by writing code that uses this crate. The more users it has,
the better tested in real-life scenarios it will be.
