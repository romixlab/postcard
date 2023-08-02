use crate::de::nibble_flavors::NibbleFlavor as NibbleFlavorDe;
use crate::error::Error;
use crate::ser::nibble_flavors::NibbleFlavor as NibbleFlavorSer;

pub struct Vlu32N(pub u32);

impl Vlu32N {
    pub fn ser(&self, flavor: &mut impl NibbleFlavorSer) -> Result<(), Error> {
        let mut val = self.0;
        let mut msb_found = false;
        let nib = (val >> 30) as u8; // get bits 31:30
        if nib != 0 {
            flavor.try_push_nib(nib | 0b1000)?;
            msb_found = true;
        }
        val <<= 2;
        for i in 0..=9 {
            if (val & (7 << 29) != 0) || msb_found {
                let nib = (val >> 29) as u8;
                if i == 9 {
                    flavor.try_push_nib(nib)?;
                } else {
                    flavor.try_push_nib(nib | 0b1000)?;
                }
                msb_found = true;
            }
            if i == 9 && !msb_found {
                flavor.try_push_nib(0)?;
            }
            val <<= 3;
        }
        Ok(())
    }

    pub fn de<'de>(flavor: &mut impl NibbleFlavorDe<'de>) -> Result<Self, Error> {
        let mut num = 0;
        for i in 0..=10 {
            let nib = flavor.try_take_nib()?;
            if i == 10 {
                // maximum 32 bits in 11 nibbles, 11th nibble should be the last
                if nib & 0b1000 != 0 {
                    return Err(Error::DeserializeBadVlu32N);
                }
            }
            num |= nib as u32 & 0b111;
            if nib & 0b1000 == 0 {
                break;
            }
            num <<= 3;
        }
        Ok(Vlu32N(num))
    }
}
