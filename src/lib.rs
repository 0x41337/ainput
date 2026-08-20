pub mod device;
pub mod multiplexer;
pub mod uinput;

pub use device::{AxisRange, OptionalAxisRange, TouchDevice, TouchDeviceError};

pub use multiplexer::{DisplaySize, Point, TouchMultiplexer};

pub use uinput::{OutputSlot, UInputError, VirtualTouchscreen};
