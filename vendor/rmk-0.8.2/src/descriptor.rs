use serde::Serialize;
use usbd_hid::descriptor::generator_prelude::*;

/// KeyboardReport describes a report and its companion descriptor that can be
/// used to send keyboard button presses to a host and receive the status of the
/// keyboard LEDs.
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
        (usage_page = KEYBOARD, usage_min = 0xE0, usage_max = 0xE7) = {
            #[packed_bits 8] #[item_settings data,variable,absolute] modifier=input;
        };
        (logical_min = 0,) = {
            #[item_settings constant,variable,absolute] reserved=input;
        };
        (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
            #[packed_bits 5] #[item_settings data,variable,absolute] leds=output;
        };
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xDD) = {
            #[item_settings data,array,absolute] keycodes=input;
        };
    }
)]
#[allow(dead_code)]
#[derive(Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyboardReport {
    pub modifier: u8, // ModifierCombination
    pub reserved: u8,
    pub leds: u8, // LedIndicator
    pub keycodes: [u8; 6],
}

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0xFF60, usage = 0x61) = {
        (usage = 0x62, logical_min = 0x0) = {
            #[item_settings data,variable,absolute] input_data=input;
        };
        (usage = 0x63, logical_min = 0x0) = {
            #[item_settings data,variable,absolute] output_data=output;
        };
    }
)]
#[derive(Default)]
pub struct ViaReport {
    pub(crate) input_data: [u8; 32],
    pub(crate) output_data: [u8; 32],
}

/// Predefined report ids for composite hid report.
/// Should be same with `#[gen_hid_descriptor]`
/// DO NOT EDIT
#[repr(u8)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]

pub enum CompositeReportType {
    #[default]
    None = 0x00,
    Mouse = 0x01,
    Media = 0x02,
    System = 0x03,
}

impl CompositeReportType {
    fn from_u8(report_id: u8) -> Self {
        match report_id {
            0x01 => Self::Mouse,
            0x02 => Self::Media,
            0x03 => Self::System,
            _ => Self::None,
        }
    }
}

pub const MOUSE_RESOLUTION_MULTIPLIER_REPORT: u8 = 0xFF;

pub const COMPOSITE_REPORT_DESCRIPTOR: &[u8] = &[
    // Mouse application collection, report ID 1.
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x09, 0x01, //   Usage (Pointer)
    0xA1, 0x00, //   Collection (Physical)
    0x05, 0x09, //     Usage Page (Button)
    0x19, 0x01, //     Usage Minimum (1)
    0x29, 0x08, //     Usage Maximum (8)
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x95, 0x08, //     Report Count (8)
    0x75, 0x01, //     Report Size (1)
    0x81, 0x02, //     Input (Data,Var,Abs)
    0x05, 0x01, //     Usage Page (Generic Desktop)
    0x09, 0x30, //     Usage (X)
    0x09, 0x31, //     Usage (Y)
    0x95, 0x02, //     Report Count (2)
    0x75, 0x08, //     Report Size (8)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x81, 0x06, //     Input (Data,Var,Rel)
    // Vertical wheel logical collection. Resolution feature shares the mouse report ID.
    0xA1, 0x02, //       Collection (Logical)
    0x09, 0x48, //         Usage (Resolution Multiplier)
    0x15, 0x00, //         Logical Minimum (0)
    0x25, 0x0F, //         Logical Maximum (15)
    0x35, 0x01, //         Physical Minimum (1)
    0x45, 0x10, //         Physical Maximum (16)
    0x75, 0x04, //         Report Size (4)
    0x95, 0x01, //         Report Count (1)
    0xA4, //               Push
    0xB1, 0x02, //         Feature (Data,Var,Abs)
    0x09, 0x38, //         Usage (Wheel)
    0x35, 0x00, //         Physical Minimum (0)
    0x45, 0x00, //         Physical Maximum (0)
    0x15, 0x81, //         Logical Minimum (-127)
    0x25, 0x7F, //         Logical Maximum (127)
    0x75, 0x08, //         Report Size (8)
    0x95, 0x01, //         Report Count (1)
    0x81, 0x06, //         Input (Data,Var,Rel)
    0xC0, //             End Collection
    // Horizontal wheel logical collection. Shares the mouse feature report.
    0xA1, 0x02, //       Collection (Logical)
    0x05, 0x01, //         Usage Page (Generic Desktop)
    0x09, 0x48, //         Usage (Resolution Multiplier)
    0xB4, //               Pop
    0xB1, 0x02, //         Feature (Data,Var,Abs)
    0x35, 0x00, //         Physical Minimum (0)
    0x45, 0x00, //         Physical Maximum (0)
    0x05, 0x0C, //         Usage Page (Consumer)
    0x0A, 0x38, 0x02, //   Usage (AC Pan)
    0x15, 0x81, //         Logical Minimum (-127)
    0x25, 0x7F, //         Logical Maximum (127)
    0x75, 0x08, //         Report Size (8)
    0x95, 0x01, //         Report Count (1)
    0x81, 0x06, //         Input (Data,Var,Rel)
    0xC0, //             End Collection
    0xC0, //           End Collection (Physical)
    0xC0, //         End Collection (Application)
    // Consumer control, report ID 2.
    0x05, 0x0C, // Usage Page (Consumer)
    0x09, 0x01, // Usage (Consumer Control)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x02, //   Report ID (2)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0x14, 0x05, // Logical Maximum (0x514)
    0x19, 0x00, //   Usage Minimum (0)
    0x2A, 0x14, 0x05, // Usage Maximum (0x514)
    0x75, 0x10, //   Report Size (16)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x00, //   Input (Data,Array,Abs)
    0xC0, //       End Collection
    // System control, report ID 3.
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x80, // Usage (System Control)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x03, //   Report ID (3)
    0x19, 0x81, //   Usage Minimum (0x81)
    0x29, 0xB7, //   Usage Maximum (0xB7)
    0x15, 0x01, //   Logical Minimum (1)
    0x26, 0xB7, 0x00, // Logical Maximum (0xB7)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x00, //   Input (Data,Array,Abs)
    0xC0, //       End Collection
];

pub const COMPOSITE_REPORT_DESCRIPTOR_LEN: usize = COMPOSITE_REPORT_DESCRIPTOR.len();

/// A composite hid report which contains mouse, consumer, system reports.
/// Report id is used to distinguish from them.
#[derive(Default, Serialize)]
pub struct CompositeReport {
    pub(crate) buttons: u8, // MouseButtons
    pub(crate) x: i8,
    pub(crate) y: i8,
    pub(crate) wheel: i8, // Scroll down (negative) or up (positive) this many units
    pub(crate) pan: i8,   // Scroll left (negative) or right (positive) this many units
    pub(crate) media_usage_id: u16,
    pub(crate) system_usage_id: u8,
}

impl usbd_hid::descriptor::SerializedDescriptor for CompositeReport {
    fn desc() -> &'static [u8] {
        COMPOSITE_REPORT_DESCRIPTOR
    }
}
