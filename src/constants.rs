pub const LockedFundRequest: u8 = 0u8; // cross chain request caused by fund locking on src chain
pub const UnLockedWithdrawRequest: u8 = 1u8; // unlocking fund on other chain caused by withdraw from digichain
pub const UnLockedFailedRequest: u8 = 2u8; // unlocking blocked fund on other chain if locked fund request failed on digichain

// 1 -> withdraw
// 2 -> ack received
