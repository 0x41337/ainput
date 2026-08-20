pub mod device;
pub mod human;
pub mod multiplexer;
pub mod touch;
pub mod uinput;

pub use device::{AxisRange, OptionalAxisRange, TouchDevice, TouchDeviceError};

pub use human::HumanProfile;

pub use multiplexer::{DisplaySize, Point, TouchMultiplexer, TouchMultiplexerBuilder};

pub use touch::{TouchAction, TouchContext, TouchController, TouchProfile};

pub use uinput::{OutputSlot, UInputError, VirtualTouchscreen};
