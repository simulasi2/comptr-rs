//! The main feature of this crate is the `ComPtr` struct, which is used to manage a non-null pointer to a COM interface.

#![feature(shared)]

#![cfg(windows)]
#![deny(warnings, missing_docs)]

extern crate winapi;
use winapi::um::unknwnbase::IUnknown;
use winapi::Interface;

use std::{ptr, mem, fmt, ops, convert};

/// A pointer to a COM interface.
pub struct ComPtr<T>(ptr::Shared<T>);

impl<T> ComPtr<T> {
	/// Constructs a COM pointer by calling an initialization callback.
	///
	/// The callback receives a reference to a `*mut T`, and **must** initialize it to some non-null value.
	/// Use `try_new_with` if initialization can fail.
	pub fn new_with<F>(initializer: F) -> Self
		where F: FnOnce(&mut *mut T) {
		let mut ptr = ptr::null_mut();

		initializer(&mut ptr);

		Self::from_raw(ptr)
	}

	/// Tries to construct a COM pointer by calling an initialization callback that could fail.
	///
	/// The callback either returns `None`, meaning `ptr` must be initialized, or `Some(error)`, meaning an error occured.
	pub fn try_new_with<F, E>(initializer: F) -> Result<Self, E>
		where F: FnOnce(&mut *mut T) -> Option<E> {
		let mut ptr = ptr::null_mut();

		let error = initializer(&mut ptr);

		match error {
			Some(error) => Err(error),
			None => Ok(Self::from_raw(ptr))
		}
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
			Some(ComPtr::from_raw_unchecked(unsafe { mem::transmute(ptr) }))
		} else {
			None
		}
	}

	/// Returns a mutable reference to the COM interface.
	pub fn to_raw(&self) -> &mut T {
		// We don't need any checks because of the class invariant.
		unsafe {
			&mut *self.0.as_ptr()
		}
	}

	/// Initialize from a COM pointer, checking to make sure it's not null.
	fn from_raw(ptr: *mut T) -> Self {
		assert_ne!(ptr, ptr::null_mut());

		Self::from_raw_unchecked(ptr)
	}

	/// Initialize from a raw COM pointer, already checked to be non-null.
	/// Warning: make sure `ptr` is not null, otherwise you could break the invariant.
	fn from_raw_unchecked(ptr: *mut T) -> Self {
		unsafe {
			ComPtr(ptr::Shared::new(ptr))
		}
	}

	/// Up-casts the pointer to IUnknown.
	fn as_unknown(&self) -> &mut IUnknown {
		unsafe {
			mem::transmute(self.0)
		}
	}
}

impl<T> Drop for ComPtr<T> {
	fn drop(&mut self) {
		unsafe {
			self.as_unknown().Release();
		}
	}
}

impl<T> fmt::Debug for ComPtr<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "ComPtr({:?})", self.0.as_ptr())
	}
}

impl<T> Clone for ComPtr<T> {
	fn clone(&self) -> Self {
		unsafe {
			self.as_unknown().AddRef();
		}

		// Safe to call because we know the original was non-null.
		Self::from_raw_unchecked(self.to_raw())
	}
}

impl<T> ops::Deref for ComPtr<T> {
	type Target = T;
	fn deref(&self) -> &T {
		self.to_raw()
	}
}

impl<T> ops::DerefMut for ComPtr<T> {
	fn deref_mut(&mut self) -> &mut T {
		self.to_raw()
	}
}

impl<T> convert::AsMut<*mut T> for ComPtr<T> {
	fn as_mut(&mut self) -> &mut *mut T {
		unsafe {
			mem::transmute(self)
		}
	}
}
