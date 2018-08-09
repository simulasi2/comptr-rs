//! The main feature of this crate is the `ComPtr` struct, which is used to manage a non-null pointer to a COM interface.
//!
//! # Using `ComPtr`
//!
//! By default, a `ComPtr` interface is not `Sync` or `Send`:
//!
//! - If you know your abstraction uses a thread-safe interface,
//! you can use `unsafe impl Sync for ... { }` to make your type `Sync`.
//!
//! - If you know your interface will only be created in a
//! [Multithreaded COM Apartment](https://msdn.microsoft.com/en-us/library/windows/desktop/ms693421(v=vs.85).aspx),
//! you can use `unsafe impl Send for ... { }` to make your type `Send`.

#![cfg(windows)]
#![deny(warnings, missing_docs)]

#[cfg_attr(test, macro_use)]
extern crate winapi;
use winapi::{um::unknwnbase::IUnknown, Interface};

use std::{convert, fmt, mem, ops, ptr};

/// A pointer to a COM interface.
///
/// The pointer owns a reference to the COM interface, meaning the COM object
/// cannot be destroyed until the last `ComPtr` using it is destroyed.
// TODO: use `Shared` once it becomes stable.
pub struct ComPtr<T: Interface>(ptr::NonNull<T>);

impl<T: Interface> ComPtr<T> {
    /// Constructs a `ComPtr` from a non-null raw pointer, asserting it to be non-null.
    pub fn new(raw_pointer: *mut T) -> Self {
        let ptr =
            ptr::NonNull::new(raw_pointer).expect("Tried to create `ComPtr` from a null pointer");

        ComPtr(ptr)
    }

    /// Constructs a `ComPtr` from a non-null raw pointer, without checking it to be non-null.
    ///
    /// Warning: it's important that you ensure that `raw_pointer` isn't null.
    pub unsafe fn new_unchecked(raw_pointer: *mut T) -> Self {
        let ptr = ptr::NonNull::new_unchecked(raw_pointer);

        ComPtr(ptr)
    }

    /// Retrieves a pointer to another interface implemented by this COM object.
    pub fn query_interface<U>(&self) -> Option<ComPtr<U>>
    where
        U: Interface,
    {
        // Pointer to store the retrieved interface.
        let mut ptr = ptr::null_mut();

        unsafe {
            // No checking of the return type because:
            // - `&mut ptr` cannot be null, so we cannot get `E_POINTER`.
            // - if we get `E_NOINTERFACE`, then `ptr` will be set to `NULL`, so that's what we check for.
            self.as_unknown().QueryInterface(&U::uuidof(), &mut ptr);
        }

        ptr::NonNull::new(ptr as *mut U).map(|ptr| ComPtr(ptr))
    }

    /// Up-casts in the inheritance hierarchy.
    ///
    /// Rust does not understand inheritance, therefore this function has to be manually called.
    pub fn upcast<U: Interface>(&self) -> &ComPtr<U>
    where
        T: ops::Deref<Target = U>,
    {
        unsafe { mem::transmute(self) }
    }

    /// Gets a mutable reference to this interface.
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut **mem::transmute::<&Self, &*mut T>(self) }
    }

    // Up-casts the pointer to IUnknown.
    //
    // Note: clients of the library can just call these methods on the interface.
    // However, being in generic code, and without having any way to have IUnknown as a trait bound, we need this method.
    fn as_unknown(&self) -> &mut IUnknown {
        unsafe { mem::transmute(self.get_mut()) }
    }
}

impl<T: Interface> Drop for ComPtr<T> {
    fn drop(&mut self) {
        unsafe {
            self.as_unknown().Release();
        }
    }
}

impl<T: Interface> Clone for ComPtr<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.as_unknown().AddRef();
        }

        // Safe to call because we know the original was non-null.
        unsafe { Self::new_unchecked(self.get_mut()) }
    }
}

impl<T: Interface> fmt::Debug for ComPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ComPtr({:p})", self.get_mut())
    }
}

impl<T: Interface> fmt::Pointer for ComPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:p}", self.get_mut())
    }
}

impl<T: Interface> ops::Deref for ComPtr<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get_mut()
    }
}

impl<T: Interface> convert::Into<*mut T> for ComPtr<T> {
    /// Returns the containing pointer, without calling `Release`.
    ///
    /// Warning: this function can be used to leak memory.
    fn into(self) -> *mut T {
        unsafe { mem::transmute(self) }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

    // Create a fake interface to test ComPtr.
    RIDL! {
        #[uuid(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11)]
        interface TestInterface(TestInterfaceVtbl): IUnknown(IUnknownVtbl) {
            fn test_function() -> u32,
        }
    }

    fn create_interface(output: *mut *mut TestInterface) {
        // Note: this function leaks memory in many places.
        // Fortunately it is reclaimed by Windows when the tests end.

        let mut test_interface: Box<TestInterface> = Box::new(unsafe { mem::zeroed() });

        let mut vtbl: Box<TestInterfaceVtbl> = Box::new(unsafe { mem::zeroed() });

        // Function pointers for IUnknown.
        {
            let unkn_vtbl = &mut vtbl.parent;

            use winapi::ctypes::c_void;
            use winapi::shared::guiddef::REFIID;

            unsafe extern "system" fn query_interface(
                this: *mut IUnknown,
                _id: REFIID,
                output: *mut *mut c_void,
            ) -> i32 {
                // We know the only ID could ever by the ID of IUnknown, or of this very interface.
                // Therefore we can return the same pointer.
                *output = mem::transmute(this);
                0
            }

            unsafe extern "system" fn add_ref(_this: *mut IUnknown) -> u32 {
                0
            }

            unsafe extern "system" fn release(_this: *mut IUnknown) -> u32 {
                0
            }

            unkn_vtbl.QueryInterface = query_interface;
            unkn_vtbl.AddRef = add_ref;
            unkn_vtbl.Release = release;
        }

        // The vtable for the actual TestInterface.
        {
            unsafe extern "system" fn test_function(_this: *mut TestInterface) -> u32 {
                1234
            }

            vtbl.test_function = test_function;
        }

        test_interface.lpVtbl = Box::into_raw(vtbl);

        unsafe {
            *output = Box::into_raw(test_interface);
        }
    }

    fn create_com_ptr() -> ComPtr<TestInterface> {
        let mut ptr = ptr::null_mut();

        create_interface(&mut ptr);

        unsafe { ComPtr::new_unchecked(ptr) }
    }

    #[test]
    fn create_and_use_interface() {
        let comptr = create_com_ptr();

        {
            let unknown = comptr.query_interface::<IUnknown>();

            let _clone = unknown.clone();
        }

        assert_eq!(unsafe { comptr.test_function() }, 1234);
    }

    #[test]
    fn debug_trait() {
        let comptr = create_com_ptr();

        println!("{:?}", comptr);
    }

    #[test]
    fn pointer_trait() {
        let comptr = create_com_ptr();

        println!("ComPtr printed as a pointer: {:p}", comptr);
    }

    #[test]
    fn non_zero_optimization() {
        let value = Some(create_com_ptr());

        assert_eq!(
            mem::size_of_val(&value),
            mem::size_of::<usize>(),
            "Non-zero optimization is not working."
        );
    }

    #[test]
    fn same_as_pointer() {
        let comptr = create_com_ptr();

        assert_eq!(mem::size_of_val(&comptr), mem::size_of::<*mut ()>());
    }

    // These tests are not supposed to compile. If they compile and run,
	// there is a problem with the way `ComPtr` is defined.
/*
	fn not_sync() {
		let ptr = std::sync::Arc::new(create_com_ptr());

		{
			let clone = ptr.clone();

			std::thread::spawn(move || {
				clone.get_mut();
			});
		}

		ptr.get_mut();
	}

	fn not_send() {
		let ptr = create_com_ptr();
		std::thread::spawn(move || {
			let _ptr = ptr;
		});
	}
*/
}
