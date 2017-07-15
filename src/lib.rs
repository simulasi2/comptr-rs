//! The main feature of this crate is the `ComPtr` struct, which is used to manage a non-null pointer to a COM interface.

#![feature(shared)]
#![cfg(windows)]
#![deny(warnings, missing_docs)]

// Macros are used in the unit tests, to generate interfaces.
#[cfg(test)]
#[macro_use] extern crate winapi;

// Normal build does not use the macros.
#[cfg(not(test))]
extern crate winapi;
use winapi::um::unknwnbase::IUnknown;
use winapi::Interface;

use std::{ptr, mem, fmt, ops};

/// A pointer to a COM interface.
///
/// The pointer owns a reference to the COM interface, meaning the COM object
/// cannot be destroyed until the last `ComPtr` using it is destroyed.
pub struct ComPtr<T>(ptr::Shared<T>);

impl<T> ComPtr<T> {
	/// Constructs a `ComPtr` from a non-null raw pointer, without checking it to be non-null.
	///
	/// Warning: it's important that you check the `raw_pointer` to not be null before calling this function.
	pub fn new(raw_pointer: *mut T) -> Self {
		ComPtr(unsafe { ptr::Shared::new(raw_pointer) })
	}

	/// Retrieves a pointer to another interface implemented by this COM object.
	pub fn query_interface<U>(&self) -> Option<ComPtr<U>>
		where U: Interface {
		// Pointer to store the retrieved interface.
		let mut ptr = ptr::null_mut();

		unsafe {
			// No checking of the return type because:
			// - `&mut ptr` cannot be null, so we cannot get `E_POINTER`.
			// - if we get `E_NOINTERFACE`, then `ptr` will be set to `NULL`, so that's what we check for.
			self.as_unknown().QueryInterface(&U::uuidof(), &mut ptr);
		}

		if ptr != ptr::null_mut() {
			// Already did the non-null check.
			Some(ComPtr::new(unsafe { mem::transmute(ptr) }))
		} else {
			None
		}
	}

	/// Returns a mutable reference to the COM interface.
	// Cannot use the `AsMut` trait because it does not work with interior mutability.
	pub fn as_mut(&self) -> &mut T {
		// We don't need any checks because of the class invariant.
		unsafe {
			&mut *self.get()
		}
	}

	/// Returns the containing pointer, without calling `Release`.
	///
	/// Warning: this function can be used to leak memory.
	pub fn into_raw(self) -> *mut T {
		let ptr = self.get();
		mem::forget(self);
		ptr
	}

	// Up-casts the pointer to IUnknown.
	//
	// Note: clients of the library can just call these methods on the interface.
	// However, being in generic code, and without having any way to have IUnknown as a trait bound, we need this method.
	fn as_unknown(&self) -> &mut IUnknown {
		unsafe {
			mem::transmute(self.as_mut())
		}
	}

	// Returns a non-owning pointer to the interface.
	//
	// Note: it's recommended to use this method instead of calling methods on the `Shared` struct,
	// since it is still unstable and its API could change.
	fn get(&self) -> *mut T {
		self.0.as_ptr()
	}
}

impl<T> Drop for ComPtr<T> {
	fn drop(&mut self) {
		unsafe {
			self.as_unknown().Release();
		}
	}
}

impl<T> Clone for ComPtr<T> {
	fn clone(&self) -> Self {
		unsafe {
			self.as_unknown().AddRef();
		}

		// Safe to call because we know the original was non-null.
		ComPtr(self.0)
	}
}

impl<T> fmt::Debug for ComPtr<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "ComPtr({:p})", self.get())
	}
}

impl<T> fmt::Pointer for ComPtr<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:p}", self.get())
	}
}

impl<T> ops::Deref for ComPtr<T> {
	type Target = T;
	fn deref(&self) -> &T {
		self.as_mut()
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

	pub fn create_interface(output: *mut *mut TestInterface) {
		// Note: this function leaks memory in many places.
		// Fortunately it is reclaimed by Windows when the tests end.

		let mut test_interface: Box<TestInterface> = Box::new(unsafe { mem::zeroed() });

		let mut vtbl: Box<TestInterfaceVtbl> = Box::new(unsafe { mem::zeroed() });

		// Function pointers for IUnknown.
		{
			let unkn_vtbl = &mut vtbl.parent;

			use winapi::ctypes::c_void;
			use winapi::shared::guiddef::REFIID;

			unsafe extern "system"
			fn query_interface(this: *mut IUnknown, _id: REFIID, output: *mut *mut c_void) -> i32 {
				// We know the only ID could ever by the ID of IUnknown, or of this very interface.
				// Therefore we can return the same pointer.
				*output = mem::transmute(this);
				0
			}

			unsafe extern "system"
			fn add_ref(_this: *mut IUnknown) -> u32 { 0 }

			unsafe extern "system"
			fn release(_this: *mut IUnknown) -> u32 { 0 }

			unkn_vtbl.QueryInterface = query_interface;
			unkn_vtbl.AddRef = add_ref;
			unkn_vtbl.Release = release;
		}

		// The vtable for the actual TestInterface.
		{
			unsafe extern "system"
			fn test_function(_this: *mut TestInterface) -> u32 {
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
		ComPtr::new({
			let mut ptr = ptr::null_mut();
			create_interface(&mut ptr);
			ptr
		})
	}

	#[test]
	fn create_and_use_interface() {
		let comptr = create_com_ptr();

		{
			let unknown = comptr.query_interface::<IUnknown>();

			let _clone = unknown.clone();
		}

		assert_eq!(unsafe { comptr.as_mut().test_function() }, 1234);
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
}
