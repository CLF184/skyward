//! The phone's secure element, as it hands its answer back through the
//! hardware interface the recovery can reach. A recovery has no framework,
//! so the OMAPI service that the phone's own programs use is not there; the
//! element's own interface is, and it is what this talks to.

// The calls and answers serve the phone's own driver and the host tests.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

#[cfg(target_os = "android")]
use anyhow::Context;
use anyhow::Result;

/// The secure element of this phone, as it registers with the service
/// directory. Qualcomm's service names the one in the chip eSE1.
pub const SERVICE: &str = "android.hardware.secure_element.ISecureElement/eSE1";
pub const INTERFACE: &str = "android.hardware.secure_element.ISecureElement";

/// What the interface numbers its methods, in the order it declares them.
pub const INIT: u32 = 1;
pub const GET_ATR: u32 = 2;
pub const IS_CARD_PRESENT: u32 = 3;
pub const TRANSMIT: u32 = 4;
pub const OPEN_LOGICAL_CHANNEL: u32 = 5;
pub const OPEN_BASIC_CHANNEL: u32 = 6;
pub const CLOSE_CHANNEL: u32 = 7;

#[cfg(target_os = "android")]
pub use android::Element;

#[cfg(target_os = "android")]
mod android {
    use super::*;
    use skyward_binder::parcel::Parcel;
    use skyward_binder::status;

    /// The secure element, as this program talks to it.
    pub struct Element<'a> {
        device: skyward_binder::driver::Handle<'a>,
    }

    impl<'a> Element<'a> {
        /// Reach the running secure element, or say that none is.
        pub fn open(binder: &'a skyward_binder::driver::Binder) -> Result<Element<'a>> {
            let device = skyward_binder::service::find(binder, SERVICE)?
                .with_context(|| format!("{SERVICE} is not running; the phone's secure element has to be started first"))?;
            Ok(Element { device })
        }

        /// The answer the element gives when it is addressed, which says
        /// which kind of card it is. Reading it changes nothing.
        pub fn atr(&self) -> Result<Vec<u8>> {
            let reply = self
                .device
                .transact(GET_ATR, &Parcel::call(INTERFACE))
                .context("ask the secure element for its answer to reset")?;
            let mut read = reply.read();
            status::ok(&mut read).context("ask the secure element for its answer to reset")?;
            // The answer is written as a value that may be absent; an empty
            // one is the element saying it has no answer.
            let present = read.i32().context("read whether the element answered")?;
            if present == 0 {
                return Ok(Vec::new());
            }
            read.blob().context("read the element's answer to reset")
        }

        /// Whether a card is in the element at all.
        pub fn present(&self) -> Result<bool> {
            let reply = self
                .device
                .transact(IS_CARD_PRESENT, &Parcel::call(INTERFACE))
                .context("ask the secure element whether a card is present")?;
            let mut read = reply.read();
            status::ok(&mut read).context("ask the secure element whether a card is present")?;
            Ok(read.i32().context("read whether a card is present")? != 0)
        }
    }
}
