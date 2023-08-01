//! # Nibble Deserialization Flavors
//!

use crate::{Error, Result};
use core::marker::PhantomData;

/// The deserialization Flavor trait
///
/// This is used as the primary way to decode serialized data from some kind of buffer,
/// or modify that data in a middleware style pattern.
///
/// See the module level docs for an example of how flavors are used.
pub trait NibbleFlavor<'de>: 'de {
    /// The remaining data of this flavor after deserializing has completed.
    ///
    /// Typically, this includes the remaining buffer that was not used for
    /// deserialization, and in cases of more complex flavors, any additional
    /// information that was decoded or otherwise calculated during
    /// the deserialization process.
    type Remainder: 'de;

    /// The source of data retrieved for deserialization.
    ///
    /// This is typically some sort of data buffer, or another Flavor, when
    /// chained behavior is desired
    type Source: 'de;

    /// Obtain the next nibble for deserialization
    fn try_take_nib(&mut self) -> Result<u8>;

    /// Obtain the next byte for deserialization
    fn try_take_u8(&mut self) -> Result<u8>;

    /// Attempt to take the next `ct` bytes from the serialized message
    fn try_take_n(&mut self, ct: usize) -> Result<&'de [u8]>;

    /// Complete the deserialization process.
    ///
    /// This is typically called separately, after the `serde` deserialization
    /// has completed.
    fn finalize(self) -> Result<Self::Remainder>;
}

/// A simple [`Flavor`] representing the deserialization from a borrowed slice
pub struct NibbleSlice<'de> {
    // This string starts with the input data and characters are truncated off
    // the beginning as data is parsed.
    pub(crate) cursor: *const u8,
    pub(crate) is_at_byte_boundary: bool,
    pub(crate) end: *const u8,
    pub(crate) _pl: PhantomData<&'de [u8]>,
}

impl<'de> NibbleSlice<'de> {
    /// Create a new [Slice] from the given buffer
    pub fn new(sli: &'de [u8]) -> Self {
        Self {
            cursor: sli.as_ptr(),
            is_at_byte_boundary: true,
            end: unsafe { sli.as_ptr().add(sli.len()) },
            _pl: PhantomData,
        }
    }

    fn align(&mut self) -> Result<()> {
        if !self.is_at_byte_boundary {
            self.try_take_nib()?;
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

impl<'de> NibbleFlavor<'de> for NibbleSlice<'de> {
    type Remainder = &'de [u8];
    type Source = &'de [u8];

    #[inline]
    fn try_take_nib(&mut self) -> Result<u8> {
        unsafe {
            if self.is_at_byte_boundary {
                self.is_at_byte_boundary = false;
                Ok(((*self.cursor) & 0xf0) >> 4)
            } else {
                self.is_at_byte_boundary = true;
                let res = Ok((*self.cursor) & 0x0f);
                self.cursor = self.cursor.add(1);
                res
            }
        }
    }

    #[inline]
    fn try_take_u8(&mut self) -> Result<u8> {
        if self.cursor == self.end {
            Err(Error::DeserializeUnexpectedEnd)
        } else {
            unsafe {
                if self.is_at_byte_boundary {
                    let res = Ok(*self.cursor);
                    self.cursor = self.cursor.add(1);
                    res
                } else {
                    let msn = *self.cursor;
                    self.cursor = self.cursor.add(1);
                    let lsn = *self.cursor;
                    Ok((msn << 4) | (lsn >> 4))
                }
            }
        }
    }

    #[inline]
    fn try_take_n(&mut self, bytes: usize) -> Result<&'de [u8]> {
        self.align()?;
        if self.nibbles_left() / 2 < bytes {
            Err(Error::DeserializeUnexpectedEnd)
        } else {
            unsafe {
                let sli = core::slice::from_raw_parts(self.cursor, bytes);
                self.cursor = self.cursor.add(bytes);
                Ok(sli)
            }
        }
    }

    /// Return the remaining (unused) bytes in the Deserializer
    fn finalize(self) -> Result<&'de [u8]> {
        let remain = (self.end as usize) - (self.cursor as usize);
        unsafe { Ok(core::slice::from_raw_parts(self.cursor, remain)) }
    }
}
