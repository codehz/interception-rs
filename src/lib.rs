pub extern crate interception_sys;

#[macro_use]
extern crate bitflags;

pub use interception_sys as raw;
pub mod bounded_slice;
pub mod scancode;

pub use bounded_slice::{BoundedSlice, BoundedSliceSource};
pub use scancode::ScanCode;

use std::char::decode_utf16;
use std::convert::{TryFrom, TryInto};
use std::default::Default;
use std::mem::MaybeUninit;
use std::ops::{Index, IndexMut};
use std::time::Duration;

pub type Device = i32;
pub type Precedence = i32;

pub enum Filter {
    MouseFilter(MouseFilter),
    KeyFilter(KeyFilter),
}

pub type Predicate = extern "C" fn(device: Device) -> bool;

bitflags! {
    pub struct MouseState: u16 {
        const LEFT_BUTTON_DOWN = 1;
        const LEFT_BUTTON_UP = 2;

        const RIGHT_BUTTON_DOWN = 4;
        const RIGHT_BUTTON_UP = 8;

        const MIDDLE_BUTTON_DOWN = 16;
        const MIDDLE_BUTTON_UP = 32;

        const BUTTON_4_DOWN = 64;
        const BUTTON_4_UP = 128;

        const BUTTON_5_DOWN = 256;
        const BUTTON_5_UP = 512;

        const WHEEL = 1024;
        const HWHEEL = 2048;

        // MouseFilter only
        const MOVE = 4096;
    }
}

pub type MouseFilter = MouseState;

bitflags! {
    pub struct MouseFlags: u16 {
        const MOVE_RELATIVE = 0;
        const MOVE_ABSOLUTE = 1;

        const VIRTUAL_DESKTOP = 2;
        const ATTRIBUTES_CHANGED = 4;

        const MOVE_NO_COALESCE = 8;

        const TERMSRV_SRC_SHADOW = 256;
    }
}

bitflags! {
    pub struct KeyState: u16 {
        const DOWN = 0;
        const UP = 1;

        const E0 = 2;
        const E1 = 3;

        const TERMSRV_SET_LED = 8;
        const TERMSRV_SHADOW = 16;
        const TERMSRV_VKPACKET = 32;
    }
}

bitflags! {
    pub struct KeyFilter: u16 {
        const DOWN = 1;
        const UP = 2;

        const E0 = 4;
        const E1 = 8;

        const TERMSRV_SET_LED = 16;
        const TERMSRV_SHADOW = 32;
        const TERMSRV_VKPACKET = 64;
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Stroke {
    Mouse {
        state: MouseState,
        flags: MouseFlags,
        rolling: i16,
        x: i32,
        y: i32,
        information: u32,
    },

    Keyboard {
        code: ScanCode,
        state: KeyState,
        information: u32,
    },
}

impl TryFrom<raw::InterceptionMouseStroke> for Stroke {
    type Error = &'static str;

    fn try_from(raw_stroke: raw::InterceptionMouseStroke) -> Result<Self, Self::Error> {
        let state = match MouseState::from_bits(raw_stroke.state) {
            Some(state) => state,
            None => return Err("Extra bits in raw mouse state"),
        };

        let flags = match MouseFlags::from_bits(raw_stroke.flags) {
            Some(flags) => flags,
            None => return Err("Extra bits in raw mouse flags"),
        };

        Ok(Stroke::Mouse {
            state: state,
            flags: flags,
            rolling: raw_stroke.rolling,
            x: raw_stroke.x,
            y: raw_stroke.y,
            information: raw_stroke.information,
        })
    }
}

impl TryFrom<raw::InterceptionKeyStroke> for Stroke {
    type Error = &'static str;

    fn try_from(raw_stroke: raw::InterceptionKeyStroke) -> Result<Self, Self::Error> {
        let state = match KeyState::from_bits(raw_stroke.state) {
            Some(state) => state,
            None => return Err("Extra bits in raw keyboard state"),
        };

        let code = match ScanCode::try_from(raw_stroke.code) {
            Ok(code) => code,
            Err(_) => ScanCode::Esc,
        };

        Ok(Stroke::Keyboard {
            code: code,
            state: state,
            information: raw_stroke.information,
        })
    }
}

impl TryFrom<Stroke> for raw::InterceptionMouseStroke {
    type Error = &'static str;

    fn try_from(stroke: Stroke) -> Result<Self, Self::Error> {
        if let Stroke::Mouse {
            state,
            flags,
            rolling,
            x,
            y,
            information,
        } = stroke
        {
            Ok(raw::InterceptionMouseStroke {
                state: state.bits(),
                flags: flags.bits(),
                rolling: rolling,
                x: x,
                y: y,
                information: information,
            })
        } else {
            Err("Stroke must be a mouse stroke")
        }
    }
}

impl TryFrom<Stroke> for raw::InterceptionKeyStroke {
    type Error = &'static str;

    fn try_from(stroke: Stroke) -> Result<Self, Self::Error> {
        if let Stroke::Keyboard {
            code,
            state,
            information,
        } = stroke
        {
            Ok(raw::InterceptionKeyStroke {
                code: code as u16,
                state: state.bits(),
                information: information,
            })
        } else {
            Err("Stroke must be a keyboard stroke")
        }
    }
}

pub struct Interception {
    ctx: raw::InterceptionContext,
    text_buffer: [u16; 512],
}

pub trait InterceptionBuffer<const BUFFER_SIZE: usize>
where
    Self: Index<usize, Output = Stroke>,
    Self: IndexMut<usize>,
    Self: BoundedSliceSource<Stroke, BUFFER_SIZE>,
{
    fn new() -> Self;
}

impl<const BUFFER_SIZE: usize> InterceptionBuffer<BUFFER_SIZE> for [Stroke; BUFFER_SIZE] {
    fn new() -> Self {
        unsafe { MaybeUninit::uninit().assume_init() }
    }
}

impl Interception {
    pub fn new() -> Option<Self> {
        let ctx = unsafe { raw::interception_create_context() };

        if ctx == std::ptr::null_mut() {
            return None;
        }

        Some(Interception {
            ctx,
            text_buffer: [0; 512],
        })
    }

    pub fn get_precedence(&self, device: Device) -> Precedence {
        unsafe { raw::interception_get_precedence(self.ctx, device) }
    }

    pub fn set_precedence(&self, device: Device, precedence: Precedence) {
        unsafe { raw::interception_set_precedence(self.ctx, device, precedence) }
    }

    pub fn get_filter(&self, device: Device) -> Filter {
        if is_invalid(device) {
            return Filter::KeyFilter(KeyFilter::empty());
        }

        let raw_filter = unsafe { raw::interception_get_filter(self.ctx, device) };
        if is_mouse(device) {
            let filter = match MouseFilter::from_bits(raw_filter) {
                Some(filter) => filter,
                None => MouseFilter::empty(),
            };

            Filter::MouseFilter(filter)
        } else {
            let filter = match KeyFilter::from_bits(raw_filter) {
                Some(filter) => filter,
                None => KeyFilter::empty(),
            };

            Filter::KeyFilter(filter)
        }
    }

    pub fn set_filter(&self, filter: Filter) {
        let (predicate, filter): (Predicate, u16) = match filter {
            Filter::MouseFilter(filter) => (is_mouse, filter.bits()),
            Filter::KeyFilter(filter) => (is_keyboard, filter.bits()),
        };
        self.set_filter_internal(predicate, filter)
    }

    fn set_filter_internal(&self, predicate: Predicate, filter: u16) {
        unsafe {
            let predicate = std::mem::transmute(Some(predicate));
            raw::interception_set_filter(self.ctx, predicate, filter)
        }
    }

    pub fn wait(&self) -> Device {
        unsafe { raw::interception_wait(self.ctx) }
    }

    pub fn wait_with_timeout(&self, duration: Duration) -> Device {
        let millis = match u32::try_from(duration.as_millis()) {
            Ok(m) => m,
            Err(_) => u32::MAX,
        };

        unsafe { raw::interception_wait_with_timeout(self.ctx, millis) }
    }

    pub fn send<const BUFFER_SIZE: usize>(
        &self,
        device: Device,
        strokes: &BoundedSlice<Stroke, BUFFER_SIZE>,
    ) -> i32 {
        if is_mouse(device) {
            self.send_internal::<raw::InterceptionMouseStroke, BUFFER_SIZE>(device, strokes)
        } else if is_keyboard(device) {
            self.send_internal::<raw::InterceptionKeyStroke, BUFFER_SIZE>(device, strokes)
        } else {
            0
        }
    }

    fn send_internal<T: TryFrom<Stroke>, const BUFFER_SIZE: usize>(
        &self,
        device: Device,
        strokes: &BoundedSlice<Stroke, BUFFER_SIZE>,
    ) -> i32 {
        let mut raw_strokes: [T; BUFFER_SIZE] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut len = 0usize;
        for stroke in strokes.into_iter() {
            if let Ok(raw_stroke) = T::try_from(*stroke) {
                raw_strokes[len] = raw_stroke;
                len += 1;
            }
        }
        let raw_strokes = raw_strokes.get_prefix(len);
        let ptr = raw_strokes.as_ptr();
        unsafe { raw::interception_send(self.ctx, device, std::mem::transmute(ptr), len as u32) }
    }

    pub fn receive<
        's,
        'buffer,
        Buffer: InterceptionBuffer<BUFFER_SIZE>,
        const BUFFER_SIZE: usize,
    >(
        &'s self,
        device: Device,
        buffer: &'buffer mut Buffer,
    ) -> &'buffer BoundedSlice<Stroke, BUFFER_SIZE> {
        let len = if is_mouse(device) {
            self.receive_internal::<raw::InterceptionMouseStroke, Buffer, BUFFER_SIZE>(
                device, buffer,
            )
        } else if is_keyboard(device) {
            self.receive_internal::<raw::InterceptionKeyStroke, Buffer, BUFFER_SIZE>(device, buffer)
        } else {
            0
        };
        buffer.get_prefix(len)
    }

    fn receive_internal<
        's,
        'buffer,
        T: TryInto<Stroke> + Default + Copy,
        Buffer: InterceptionBuffer<BUFFER_SIZE>,
        const BUFFER_SIZE: usize,
    >(
        &self,
        device: Device,
        buffer: &'buffer mut Buffer,
    ) -> usize {
        let mut raw_strokes: [T; BUFFER_SIZE] = unsafe { MaybeUninit::uninit().assume_init() };

        let ptr = raw_strokes.as_mut_ptr();
        let len = match u32::try_from(raw_strokes.len()) {
            Ok(l) => l,
            Err(_) => u32::MAX,
        };

        let num_read =
            unsafe { raw::interception_receive(self.ctx, device, std::mem::transmute(ptr), len) };

        let mut num_valid: usize = 0;
        for i in 0..num_read {
            if let Ok(stroke) = raw_strokes[i as usize].try_into() {
                buffer[num_valid as usize] = stroke;
                num_valid += 1;
            }
        }

        num_valid
    }

    pub fn get_hardware_id(&mut self, device: Device) -> Option<String> {
        let ptr = self.text_buffer.as_mut_ptr();
        let len = unsafe {
            raw::interception_get_hardware_id(self.ctx, device, std::mem::transmute(ptr), 1024)
        } as usize;
        if len == 0 {
            return None;
        }
        let u16str = &self.text_buffer[..(len / 2)];
        Some(
            decode_utf16(u16str.iter().cloned())
                .map(|r| r.unwrap_or('�'))
                .collect(),
        )
    }
}

impl Drop for Interception {
    fn drop(&mut self) {
        unsafe { raw::interception_destroy_context(self.ctx) }
    }
}

pub extern "C" fn is_invalid(device: Device) -> bool {
    unsafe { raw::interception_is_invalid(device) != 0 }
}

pub extern "C" fn is_keyboard(device: Device) -> bool {
    unsafe { raw::interception_is_keyboard(device) != 0 }
}

pub extern "C" fn is_mouse(device: Device) -> bool {
    unsafe { raw::interception_is_mouse(device) != 0 }
}
