use crate::pointer::NP_PtrKinds;
use crate::{PROTOCOL_VERSION, error::NP_Error};
use core::cell::UnsafeCell;
use alloc::vec::Vec;

#[derive(Debug, Copy, Clone)]
pub enum NP_Size {
    U32,
    U16
}

#[derive(Debug)]
pub struct NP_Memory {
    bytes: UnsafeCell<Vec<u8>>,
    
    pub size: NP_Size
}

const MAX_SIZE_LARGE: usize = core::u32::MAX as usize;
const MAX_SIZE_SMALL: usize = core::u16::MAX as usize;


impl<'a> NP_Memory {

    pub fn existing(bytes: Vec<u8>) -> Self {

        let size = bytes[1];
        
        NP_Memory {
            bytes: UnsafeCell::new(bytes),
            size: if size == 0 {
                NP_Size::U32
            } else {
                NP_Size::U16
            }
        }
    }

    pub fn addr_size(&self) -> usize {
        match &self.size {
            NP_Size::U32 => MAX_SIZE_LARGE,
            NP_Size::U16 => MAX_SIZE_SMALL
        }
    }

    pub fn new(capacity: Option<usize>, size: NP_Size) -> Self {
        let use_size = match capacity {
            Some(x) => x,
            None => 1024
        };

        let mut new_bytes = Vec::with_capacity(use_size);

        new_bytes.push(PROTOCOL_VERSION); // Protocol version (for breaking changes if needed later)

        match &size {
            NP_Size::U32 => {
                new_bytes.push(0); // size key (0 for U32)
                new_bytes.extend(0u32.to_be_bytes().to_vec()); // u32 HEAD for root pointer (starts at zero)
            },
            NP_Size::U16 => {
                new_bytes.push(1); // size key (1 for U16)
                new_bytes.extend(0u16.to_be_bytes().to_vec()); // u16 HEAD for root pointer (starts at zero)
            }
        }


        NP_Memory {
            bytes: UnsafeCell::new(new_bytes),
            size: size
        }
    }

    pub fn malloc(&self, bytes: Vec<u8>) -> core::result::Result<u32, NP_Error> {

        let self_bytes = unsafe { &mut *self.bytes.get() };

        let location = self_bytes.len();

        let max_sze = match self.size {
            NP_Size::U16 => { MAX_SIZE_SMALL },
            NP_Size::U32 => { MAX_SIZE_LARGE }
        };

        // not enough space left?
        if location + bytes.len() >= max_sze {
            return Err(NP_Error::new("Not enough space available in buffer!"))
        }

        self_bytes.extend(bytes);
        Ok(location as u32)
    }

    pub fn read_bytes(&self) -> &Vec<u8> {
        let self_bytes = unsafe { &*self.bytes.get() };
        self_bytes
    }

    pub fn write_bytes(&self) -> &mut Vec<u8> {
        let self_bytes = unsafe { &mut *self.bytes.get() };
        self_bytes
    }

    pub fn ptr_size(&self, ptr: &NP_PtrKinds) -> u32 {
        // Get the size of this pointer based it's kind
        match self.size {
            NP_Size::U32 => {
                match ptr {
                    NP_PtrKinds::None                                     =>   { 0u32 },
                    NP_PtrKinds::Standard  { addr: _ }                   =>    { 4u32 },
                    NP_PtrKinds::MapItem   { addr: _, key: _,  next: _ } =>    { 12u32 },
                    NP_PtrKinds::TableItem { addr: _, i:_ ,    next: _ } =>    { 9u32 },
                    NP_PtrKinds::ListItem  { addr: _, i:_ ,    next: _ } =>    { 10u32 }
                }
            },
            NP_Size::U16 => {
                match ptr {
                    NP_PtrKinds::None                                     =>   { 0u32 },
                    NP_PtrKinds::Standard  { addr: _ }                   =>    { 2u32 },
                    NP_PtrKinds::MapItem   { addr: _, key: _,  next: _ } =>    { 6u32 },
                    NP_PtrKinds::TableItem { addr: _, i:_ ,    next: _ } =>    { 5u32 },
                    NP_PtrKinds::ListItem  { addr: _, i:_ ,    next: _ } =>    { 6u32 }
                }
            }
        }
    }

    pub fn blank_ptr_bytes(&self, ptr: &NP_PtrKinds) -> Vec<u8> {
        let size = self.ptr_size(ptr);
        let mut empty_bytes = Vec::with_capacity(size as usize);
        for _x in 0..size {
            empty_bytes.push(0);
        }
        empty_bytes
    }

    pub fn set_value_address(&self, address: u32, val: u32, kind: &NP_PtrKinds) -> NP_PtrKinds {

        let addr_bytes = match self.size {
            NP_Size::U32 => val.to_be_bytes().to_vec(),
            NP_Size::U16 => (val as u16).to_be_bytes().to_vec()
        };

        let self_bytes = unsafe { &mut *self.bytes.get() };
    
        for x in 0..addr_bytes.len() {
            self_bytes[(address + x as u32) as usize] = addr_bytes[x as usize];
        }

        match kind {
            NP_PtrKinds::None => {
                NP_PtrKinds::None
            }
            NP_PtrKinds::Standard { addr: _ } => {
                NP_PtrKinds::Standard { addr: val }
            },
            NP_PtrKinds::MapItem { addr: _, key,  next  } => {
                NP_PtrKinds::MapItem { addr: val, key: *key, next: *next }
            },
            NP_PtrKinds::TableItem { addr: _, i, next  } => {
                NP_PtrKinds::TableItem { addr: val, i: *i, next: *next }
            },
            NP_PtrKinds::ListItem { addr: _, i, next  } => {
                NP_PtrKinds::ListItem { addr: val, i: *i, next: *next }
            }
        }
    }

    pub fn get_1_byte(&self, address: usize) -> Option<u8> {

        // empty value
        if address == 0 {
            return None;
        }

        let self_bytes = unsafe { &*self.bytes.get() };
 
        Some(self_bytes[address])
    }

    pub fn get_2_bytes(&self, address: usize) -> Option<&[u8; 2]> {

        // empty value
        if address == 0 {
            return None;
        }

        let self_bytes = unsafe { &*self.bytes.get() };

        if self_bytes.len() < address + 2 {
            return None;
        }

        let slice = &self_bytes[address..(address + 2)];

        Some(unsafe { &*(slice as *const [u8] as *const [u8; 2]) })
    }

    pub fn get_4_bytes(&self, address: usize) -> Option<&[u8; 4]> {

        // empty value
        if address == 0 {
            return None;
        }

        let self_bytes = unsafe { &*self.bytes.get() };

        if self_bytes.len() < address + 4 {
            return None;
        }

        let slice = &self_bytes[address..(address + 4)];

        Some(unsafe { &*(slice as *const [u8] as *const [u8; 4]) })
    }

    pub fn get_8_bytes(&self, address: usize) -> Option<&[u8; 8]> {

        // empty value
        if address == 0 {
            return None;
        }

        let self_bytes = unsafe { &*self.bytes.get() };

        if self_bytes.len() < address + 8 {
            return None;
        }

        let slice = &self_bytes[address..(address + 8)];

        Some(unsafe { &*(slice as *const [u8] as *const [u8; 8]) })
    }

    pub fn get_16_bytes(&self, address: usize) -> Option<&[u8; 16]> {

        // empty value
        if address == 0 {
            return None;
        }

        let self_bytes = unsafe { &*self.bytes.get() };

        if self_bytes.len() < address + 16 {
            return None;
        }

        let slice = &self_bytes[address..(address + 16)];

        Some(unsafe { &*(slice as *const [u8] as *const [u8; 16]) })
    }

    pub fn get_32_bytes(&self, address: usize) -> Option<&[u8; 32]> {

        // empty value
        if address == 0 {
            return None;
        }

        let self_bytes = unsafe { &*self.bytes.get() };

        if self_bytes.len() < address + 32 {
            return None;
        }

        let slice = &self_bytes[address..(address + 32)];

        Some(unsafe { &*(slice as *const [u8] as *const [u8; 32]) })
    }

    pub fn dump(self) -> Vec<u8> {
        self.bytes.into_inner()
    }
}