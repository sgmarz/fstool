//! ExFAT Timestamps
//!
//!
//! © Stephen Marz
//! 8 June 2026

/// A decoded ExFAT timestamp.
///
/// The raw on-disk format packs date and time into a single 32-bit field:
///
/// ```text
/// Bits 31–25: year offset from 1980  (0–127 → 1980–2107)
/// Bits 24–21: month                  (1–12)
/// Bits 20–16: day                    (1–31)
/// Bits 15–11: hour                   (0–23)
/// Bits 10–5:  minute                 (0–59)
/// Bits  4–0:  second / 2             (0–29 → 0–58 seconds, 2s granularity)
/// ```
///
/// The `increment_10ms` field adds 0–1990 ms on top of the 2-second slot.
/// The `utc_offset` field encodes the timezone in 15-minute increments
/// (0x00 = unknown, otherwise value × 15 min with bias).
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct ExfatTimestamp {
    pub year: u16,         // 1980–2107
    pub month: u8,         // 1–12
    pub day: u8,           // 1–31
    pub hour: u8,          // 0–23
    pub minute: u8,        // 0–59
    pub second: u8,        // 0–58 (even only from base; add increment)
    pub increment_ms: u16, // 0–1990 ms from the 10ms field
    pub utc_offset: i16,   // minutes east of UTC; i16::MIN = unknown
}

impl Into<u64> for ExfatTimestamp {
    fn into(self) -> u64 {
        let year = (self.year - 1980) as u64;
        let month = self.month as u64;
        let day = self.day as u64;
        let hour = self.hour as u64;
        let minute = self.minute as u64;
        let second = (self.second / 2) as u64;

        (year << 25) | (month << 21) | (day << 16) | (hour << 11) | (minute << 5) | second
    }
}

impl Into<ExfatTimestamp> for u64 {
    fn into(self) -> ExfatTimestamp {
        let year = ((self >> 25) & 0x7F) as u16 + 1980;
        let month = ((self >> 21) & 0x0F) as u8;
        let day = ((self >> 16) & 0x1F) as u8;
        let hour = ((self >> 11) & 0x1F) as u8;
        let minute = ((self >> 5) & 0x3F) as u8;
        let second = ((self & 0x1F) as u8) * 2;

        ExfatTimestamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
            increment_ms: 0,      // This field is not stored in the base timestamp.
            utc_offset: i16::MIN, // Unknown by default.
        }
    }
}
