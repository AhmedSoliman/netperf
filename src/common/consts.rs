use std::time::Duration;

pub const INTERNAL_PROT_BUFFER: usize = 5;
pub const PROTOCOL_TIMEOUT: Duration = Duration::from_secs(10);
pub const MB: usize = 1024 * 1024;
pub const MAX_CONTROL_MESSAGE: u32 = 20 * (MB) as u32;
pub const DEFAULT_BLOCK_SIZE: usize = 2 * MB;
// pub const MAX_BLOCK_SIZE: i32 = 1 * MB;
// pub const MAX_TCP_BUFFER: i32 = 512 * MB;
// pub const MAX_MSS: i32 = 9 * 1024;

// Protocol Constants
pub const MESSAGE_LENGTH_SIZE_BYTES: usize = std::mem::size_of::<u32>();
