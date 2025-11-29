
// pub fn convert_u16_to_u8(val: u16) -> [u8; 2] {
//
// }

use crate::errors::MessageError;

pub fn extract_u32_from_bytes(tsbuf: &Vec<u8>, start_index: usize, end_index: usize) -> Result<u32, MessageError> {
    match tsbuf.get(start_index..end_index) {
        Some(bytes) => { Ok(u32::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt32Read)?)) },
        None => { Err(MessageError::BadInt32Read) }
    }
}

pub fn extract_u16_from_bytes(tsbuf: &Vec<u8>, start_index: usize, end_index: usize) -> Result<u16, MessageError> {
    match tsbuf.get(start_index..end_index) {
        Some(bytes) => { Ok(u16::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt16Read)?)) },
        None => { Err(MessageError::BadInt16Read) }
    }
}

pub fn extract_u8_from_byte(tsbuf: &Vec<u8>, start_index: usize, end_index: usize) -> Result<u8, MessageError> {
    match tsbuf.get(start_index..end_index) {
        Some(bytes) => { Ok(u8::from_be_bytes(bytes.try_into().map_err(|_| MessageError::BadInt8Read)?)) },
        None => { Err(MessageError::BadInt8Read) }
    }
}