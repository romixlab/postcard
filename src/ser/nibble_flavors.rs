//! # Nibble based Serialization Flavors
//!
use crate::error::{Error, Result};
use core::marker::PhantomData;
use core::ops::Index;
use core::ops::IndexMut;

#[cfg(feature = "heapless")]
pub use heapless_vec::*;

#[cfg(feature = "use-std")]
pub use std_vec::*;

#[cfg(feature = "alloc")]
pub use alloc_vec::*;

#[cfg(feature = "alloc")]
extern crate alloc;

/// The serialization Flavor trait
///
/// This is used as the primary way to encode serialized data into some kind of buffer,
/// or modify that data in a middleware style pattern.
///
/// See the module level docs for an example of how flavors are used.
pub trait NibbleFlavor {
    /// The `Output` type is what this storage "resolves" to when the serialization is complete,
    /// such as a slice or a Vec of some sort.
    type Output;

    /// The try_extend() trait method can be implemented when there is a more efficient way of processing
    /// multiple bytes at once, such as copying a slice to the output, rather than iterating over one byte
    /// at a time.
    #[inline]
    fn try_extend(&mut self, data: &[u8]) -> Result<()> {
        data.iter().try_for_each(|d| self.try_push_u8(*d))
    }

    /// The try_push_u8() trait method can be used to push a single byte to be modified and/or stored
    fn try_push_u8(&mut self, data: u8) -> Result<()>;

    /// The try_push_nib() trait method can be used to push a single nibble to be modified and/or stored
    fn try_push_nib(&mut self, nib: u8) -> Result<()>;

    /// Finalize the serialization process
    fn finalize(self) -> Result<Self::Output>;
}

////////////////////////////////////////
// Slice
////////////////////////////////////////

/// The `Slice` flavor is a storage flavor, storing the serialized (or otherwise modified) bytes into a plain
/// `[u8]` slice. The `Slice` flavor resolves into a sub-slice of the original slice buffer.
pub struct NibbleSlice<'a> {
    start: *mut u8,
    cursor: *mut u8,
    is_at_byte_boundary: bool,
    end: *mut u8,
    _pl: PhantomData<&'a [u8]>,
}

impl<'a> NibbleSlice<'a> {
    /// Create a new `Slice` flavor from a given backing buffer
    pub fn new(buf: &'a mut [u8]) -> Self {
        let ptr = buf.as_mut_ptr();
        NibbleSlice {
            start: ptr,
            cursor: ptr,
            is_at_byte_boundary: true,
            end: unsafe { ptr.add(buf.len()) },
            _pl: PhantomData,
        }
    }

    fn align(&mut self) -> Result<()> {
        if !self.is_at_byte_boundary {
            self.try_push_nib(0)?;
        }
        Ok(())
    }

    fn nibbles_left(&self) -> usize {
        let bytes_remain = (self.end as usize) - (self.cursor as usize);
        if self.is_at_byte_boundary {
            bytes_remain * 2
        } else {
            bytes_remain * 2 - 1
        }
    }
}

impl<'a> NibbleFlavor for NibbleSlice<'a> {
    type Output = &'a mut [u8];

    #[inline(always)]
    fn try_push_u8(&mut self, byte: u8) -> Result<()> {
        if self.cursor == self.end {
            Err(Error::SerializeBufferFull)
        } else {
            unsafe {
                if self.is_at_byte_boundary {
                    self.cursor.write(byte);
                    self.cursor = self.cursor.add(1);
                } else {
                    self.cursor.write(self.cursor.read() | (byte >> 4));
                    self.cursor = self.cursor.add(1);
                    if self.cursor == self.end {
                        return Err(Error::SerializeBufferFull);
                    }
                    self.cursor.write(byte << 4);
                }
            }
            Ok(())
        }
    }

    fn try_push_nib(&mut self, nib: u8) -> Result<()> {
        if self.cursor == self.end {
            Err(Error::SerializeBufferFull)
        } else {
            unsafe {
                let mut b = self.cursor.read();
                if self.is_at_byte_boundary {
                    b &= 0b0000_1111;
                    b |= nib << 4;
                    self.is_at_byte_boundary = false;
                } else {
                    b &= 0b1111_0000;
                    b |= nib & 0b0000_1111;
                    self.is_at_byte_boundary = true;
                    self.cursor = self.cursor.add(1);
                }
                self.cursor.write(b);
            }
            Ok(())
        }
    }

    #[inline(always)]
    fn try_extend(&mut self, bytes: &[u8]) -> Result<()> {
        self.align()?;
        if self.nibbles_left() < bytes.len() * 2 {
            Err(Error::SerializeBufferFull)
        } else {
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), self.cursor, bytes.len());
                self.cursor = self.cursor.add(bytes.len());
            }
            Ok(())
        }
    }

    fn finalize(self) -> Result<Self::Output> {
        let used = (self.cursor as usize) - (self.start as usize);
        let sli = unsafe { core::slice::from_raw_parts_mut(self.start, used) };
        Ok(sli)
    }
}

#[cfg(feature = "heapless")]
mod heapless_vec {
    use super::NibbleFlavor;
    use crate::{Error, Result};
    use heapless::Vec;

    ////////////////////////////////////////
    // HVec
    ////////////////////////////////////////

    /// The `HVec` flavor is a wrapper type around a `heapless::Vec`. This is a stack
    /// allocated data structure, with a fixed maximum size and variable amount of contents.
    pub struct NibbleHVec<const B: usize> {
        /// the contained data buffer
        vec: Vec<u8, B>,
        is_at_byte_boundary: bool,
    }

    impl<const B: usize> Default for NibbleHVec<B> {
        fn default() -> Self {
            Self {
                vec: Default::default(),
                is_at_byte_boundary: true,
            }
        }
    }

    impl<const B: usize> NibbleHVec<B> {
        /// Create a new, currently empty, [heapless::Vec] to be used for storing serialized
        /// output data.
        pub fn new() -> Self {
            Self::default()
        }

        fn align(&mut self) -> Result<()> {
            if !self.is_at_byte_boundary {
                self.try_push_nib(0)?;
            }
            Ok(())
        }
    }

    impl<const B: usize> NibbleFlavor for NibbleHVec<B> {
        type Output = Vec<u8, B>;

        #[inline(always)]
        fn try_extend(&mut self, bytes: &[u8]) -> Result<()> {
            self.align()?;
            self.vec
                .extend_from_slice(bytes)
                .map_err(|_| Error::SerializeBufferFull)
        }

        #[inline(always)]
        fn try_push_u8(&mut self, byte: u8) -> Result<()> {
            if self.is_at_byte_boundary {
                self.vec.push(byte).map_err(|_| Error::SerializeBufferFull)
            } else {
                self.try_push_nib(byte >> 4)?;
                self.try_push_nib(byte & 0b0000_1111)
            }
        }

        fn try_push_nib(&mut self, nib: u8) -> Result<()> {
            if let Some(b) = self.vec.last_mut() {
                if self.is_at_byte_boundary {
                    self.vec
                        .push(nib << 4)
                        .map_err(|_| Error::SerializeBufferFull)?;
                    self.is_at_byte_boundary = false;
                } else {
                    *b |= nib & 0b0000_1111;
                    self.is_at_byte_boundary = true;
                }
                Ok(())
            } else {
                self.is_at_byte_boundary = false;
                self.vec
                    .push(nib << 4)
                    .map_err(|_| Error::SerializeBufferFull)
            }
        }

        fn finalize(self) -> Result<Vec<u8, B>> {
            Ok(self.vec)
        }
    }
}

#[cfg(feature = "use-std")]
mod std_vec {
    /// The `StdVec` flavor is a wrapper type around a `std::vec::Vec`.
    ///
    /// This type is only available when the (non-default) `use-std` feature is active
    pub type StdVec = super::alloc_vec::AllocVec;
}

#[cfg(feature = "alloc")]
mod alloc_vec {
    extern crate alloc;
    use super::Flavor;
    use super::Index;
    use super::IndexMut;
    use crate::Result;
    use alloc::vec::Vec;

    /// The `AllocVec` flavor is a wrapper type around an [alloc::vec::Vec].
    ///
    /// This type is only available when the (non-default) `alloc` feature is active
    #[derive(Default)]
    pub struct AllocVec {
        /// The vec to be used for serialization
        vec: Vec<u8>,
    }

    impl AllocVec {
        /// Create a new, currently empty, [alloc::vec::Vec] to be used for storing serialized
        /// output data.
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl Flavor for AllocVec {
        type Output = Vec<u8>;

        #[inline(always)]
        fn try_extend(&mut self, data: &[u8]) -> Result<()> {
            self.vec.extend_from_slice(data);
            Ok(())
        }

        #[inline(always)]
        fn try_push(&mut self, data: u8) -> Result<()> {
            self.vec.push(data);
            Ok(())
        }

        fn finalize(self) -> Result<Self::Output> {
            Ok(self.vec)
        }
    }

    impl Index<usize> for AllocVec {
        type Output = u8;

        #[inline]
        fn index(&self, idx: usize) -> &u8 {
            &self.vec[idx]
        }
    }

    impl IndexMut<usize> for AllocVec {
        #[inline]
        fn index_mut(&mut self, idx: usize) -> &mut u8 {
            &mut self.vec[idx]
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Modification Flavors
////////////////////////////////////////////////////////////////////////////////

/// The `Size` flavor is a measurement flavor, which accumulates the number of bytes needed to
/// serialize the data.
///
/// ```
/// use postcard::{serialize_with_flavor, ser_flavors};
///
/// let value = false;
/// let size = serialize_with_flavor(&value, ser_flavors::Size::default()).unwrap();
///
/// assert_eq!(size, 1);
/// ```
#[derive(Default)]
pub struct NibbleSize {
    size_nibbles: usize,
}

impl NibbleFlavor for NibbleSize {
    type Output = usize;

    #[inline(always)]
    fn try_push_u8(&mut self, _b: u8) -> Result<()> {
        self.size_nibbles += 2;
        Ok(())
    }

    fn try_push_nib(&mut self, _nib: u8) -> Result<()> {
        self.size_nibbles += 1;
        Ok(())
    }

    #[inline(always)]
    fn try_extend(&mut self, b: &[u8]) -> Result<()> {
        self.size_nibbles += b.len() * 2;
        Ok(())
    }

    fn finalize(self) -> Result<Self::Output> {
        Ok(self.size_nibbles)
    }
}
