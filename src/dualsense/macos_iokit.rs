use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr, thread,
    time::Duration,
};

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::{
    dualsense::{
        is_supported_dualsense, AdaptiveTriggerEffect, DeviceInfo, DualSenseControl, HapticFrame,
        SONY_VENDOR_ID,
    },
    model::{Button, GamepadState, Rgb, StickState},
};

type Boolean = u8;
type CFIndex = isize;
type CFOptionFlags = u32;
type CFAllocatorRef = *const c_void;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFNumberRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFSetRef = *const c_void;
type IOOptionBits = u32;
type IOReturn = i32;
type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOHIDReportType = CFIndex;

#[repr(C)]
struct CFDictionaryKeyCallBacks {
    _private: [usize; 6],
}

#[repr(C)]
struct CFDictionaryValueCallBacks {
    _private: [usize; 5],
}

const K_CF_NUMBER_INT_TYPE: c_int = 9;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_IOHID_OPTIONS_TYPE_NONE: IOOptionBits = 0;
const K_IOHID_REPORT_TYPE_INPUT: IOHIDReportType = 0;
const K_IOHID_REPORT_TYPE_OUTPUT: IOHIDReportType = 1;

const DS_USB_INPUT_REPORT_ID: u8 = 0x01;
const DS_USB_INPUT_REPORT_LEN: usize = 64;
const DS_BT_INPUT_REPORT_ID: u8 = 0x31;
const DS_BT_INPUT_REPORT_LEN: usize = 78;
const DS_USB_OUTPUT_REPORT_ID: u8 = 0x02;
const DS_USB_OUTPUT_REPORT_LEN: usize = 63;
const DS_BT_OUTPUT_REPORT_ID: u8 = 0x31;
const DS_BT_OUTPUT_REPORT_LEN: usize = 78;
const DS_BT_OUTPUT_TAG: u8 = 0x10;
const DS_BT_CRC_SEED: u8 = 0xa2;

const DS_OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION: u8 = 1 << 0;
const DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_EFFECT: u8 = 1 << 2;
const DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_EFFECT: u8 = 1 << 3;
const DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE: u8 = 1 << 2;
const DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2: u8 = 1 << 2;
const DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_OUT: u8 = 1 << 1;

const OFFSET_VALID_FLAG0: usize = 0;
const OFFSET_VALID_FLAG1: usize = 1;
const OFFSET_MOTOR_RIGHT: usize = 2;
const OFFSET_MOTOR_LEFT: usize = 3;
const OFFSET_RIGHT_TRIGGER_EFFECT: usize = 10;
const OFFSET_LEFT_TRIGGER_EFFECT: usize = 21;
const OFFSET_VALID_FLAG2: usize = 38;
const OFFSET_LIGHTBAR_SETUP: usize = 41;
const OFFSET_LIGHTBAR_RED: usize = 44;
const OFFSET_LIGHTBAR_GREEN: usize = 45;
const OFFSET_LIGHTBAR_BLUE: usize = 46;

const OFFSET_INPUT_LEFT_X: usize = 0;
const OFFSET_INPUT_LEFT_Y: usize = 1;
const OFFSET_INPUT_RIGHT_X: usize = 2;
const OFFSET_INPUT_RIGHT_Y: usize = 3;
const OFFSET_INPUT_LEFT_TRIGGER: usize = 4;
const OFFSET_INPUT_RIGHT_TRIGGER: usize = 5;
const OFFSET_INPUT_BUTTONS0: usize = 7;
const OFFSET_INPUT_BUTTONS1: usize = 8;
const OFFSET_INPUT_BUTTONS2: usize = 9;
const OFFSET_INPUT_BATTERY: usize = 52;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
    static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;

    fn CFRelease(cf: CFTypeRef);
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFNumberCreate(
        allocator: CFAllocatorRef,
        the_type: c_int,
        value_ptr: *const c_void,
    ) -> CFNumberRef;
    fn CFNumberGetValue(number: CFNumberRef, the_type: c_int, value_ptr: *mut c_void) -> Boolean;
    fn CFStringCreateWithCString(
        allocator: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> Boolean;
    fn CFDictionaryCreate(
        allocator: CFAllocatorRef,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: CFIndex,
        key_callbacks: *const CFDictionaryKeyCallBacks,
        value_callbacks: *const CFDictionaryValueCallBacks,
    ) -> CFDictionaryRef;
    fn CFSetGetCount(the_set: CFSetRef) -> CFIndex;
    fn CFSetGetValues(the_set: CFSetRef, values: *mut *const c_void);
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: CFOptionFlags) -> IOHIDManagerRef;
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: IOOptionBits) -> IOReturn;
    fn IOHIDManagerClose(manager: IOHIDManagerRef, options: IOOptionBits) -> IOReturn;
    fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;
    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;
    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: IOOptionBits) -> IOReturn;
    fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;
    fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        report_type: IOHIDReportType,
        report_id: CFIndex,
        report: *mut u8,
        report_length: *mut CFIndex,
    ) -> IOReturn;
    fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        report_type: IOHIDReportType,
        report_id: CFIndex,
        report: *const u8,
        report_length: CFIndex,
    ) -> IOReturn;
}

pub struct DualSenseBackend {
    manager: IOHIDManagerRef,
    devices: Vec<HidDevice>,
    sequence: u8,
    input_sequence: u64,
}

impl DualSenseBackend {
    pub fn new() -> Result<Self> {
        let manager = unsafe { IOHIDManagerCreate(ptr::null(), K_IOHID_OPTIONS_TYPE_NONE) };
        ensure!(!manager.is_null(), "IOHIDManagerCreate returned null");

        set_vendor_matching(manager).context("failed to install IOKit matching dictionary")?;

        let open_result = unsafe { IOHIDManagerOpen(manager, K_IOHID_OPTIONS_TYPE_NONE) };
        if open_result != 0 {
            unsafe { CFRelease(manager as CFTypeRef) };
            bail!("IOHIDManagerOpen returned {}", format_ioreturn(open_result));
        }

        let mut backend = Self {
            manager,
            devices: Vec::new(),
            sequence: 0,
            input_sequence: 0,
        };
        backend.refresh()?;
        Ok(backend)
    }

    fn next_sequence(&mut self) -> u8 {
        let sequence = self.sequence & 0x0f;
        self.sequence = self.sequence.wrapping_add(1) & 0x0f;
        sequence
    }

    fn selected_device_mut(&mut self, index: usize) -> Result<&mut HidDevice> {
        self.devices
            .get_mut(index)
            .ok_or_else(|| anyhow!("no DualSense device selected"))
    }

    fn selected_transport(&self, index: usize) -> Result<TransportKind> {
        self.devices
            .get(index)
            .map(|device| {
                if device.info.is_bluetooth() {
                    TransportKind::Bluetooth
                } else {
                    TransportKind::Usb
                }
            })
            .ok_or_else(|| anyhow!("no DualSense device selected"))
    }

    fn send_output_report(&mut self, index: usize, report: &[u8]) -> Result<()> {
        ensure!(!report.is_empty(), "empty HID output report");
        let device = self.selected_device_mut(index)?;
        device.ensure_open()?;

        let report_id = CFIndex::from(report[0]);
        let result = unsafe {
            IOHIDDeviceSetReport(
                device.raw,
                K_IOHID_REPORT_TYPE_OUTPUT,
                report_id,
                report.as_ptr(),
                report.len() as CFIndex,
            )
        };

        if result != 0 {
            bail!(
                "IOHIDDeviceSetReport 0x{report_id:02x} for {} returned {}",
                device.info.name,
                format_ioreturn(result)
            );
        }

        Ok(())
    }
}

impl DualSenseControl for DualSenseBackend {
    fn refresh(&mut self) -> Result<()> {
        self.devices.clear();

        let device_set = unsafe { IOHIDManagerCopyDevices(self.manager) };
        if device_set.is_null() {
            return Ok(());
        }

        let count = unsafe { CFSetGetCount(device_set) };
        if count <= 0 {
            unsafe { CFRelease(device_set as CFTypeRef) };
            return Ok(());
        }

        let mut values = vec![ptr::null(); count as usize];
        unsafe { CFSetGetValues(device_set, values.as_mut_ptr()) };

        for value in values {
            let raw = value as IOHIDDeviceRef;
            if raw.is_null() {
                continue;
            }

            let vendor_id = read_number_property(raw, "VendorID").unwrap_or(0);
            let product_id = read_number_property(raw, "ProductID").unwrap_or(0);
            if !is_supported_dualsense(vendor_id, product_id) {
                continue;
            }

            let name = read_string_property(raw, "Product")
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "DualSense Wireless Controller".to_string());
            let transport = read_string_property(raw, "Transport")
                .filter(|transport| !transport.trim().is_empty())
                .unwrap_or_else(|| "USB".to_string());

            let retained = unsafe { CFRetain(value) } as IOHIDDeviceRef;
            if retained.is_null() {
                continue;
            }

            let info = DeviceInfo {
                name,
                vendor_id,
                product_id,
                transport,
            };
            self.devices.push(HidDevice::new(retained, info));
        }

        unsafe { CFRelease(device_set as CFTypeRef) };
        Ok(())
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        self.devices
            .iter()
            .map(|device| device.info.clone())
            .collect()
    }

    fn set_lightbar(&mut self, index: usize, color: Rgb) -> Result<()> {
        let transport = self.selected_transport(index)?;

        let setup = build_output_report(
            transport,
            self.next_sequence(),
            None,
            None,
            LightbarSetup::EnableLightOut,
            None,
        );
        self.send_output_report(index, &setup)?;
        thread::sleep(Duration::from_millis(20));

        let report = build_output_report(
            transport,
            self.next_sequence(),
            Some(color),
            None,
            LightbarSetup::None,
            None,
        );
        self.send_output_report(index, &report)
    }

    fn pulse_haptics(
        &mut self,
        index: usize,
        left: u8,
        right: u8,
        audio_haptics: bool,
    ) -> Result<()> {
        self.play_haptics(index, &[HapticFrame::new(left, right, 180)], audio_haptics)
    }

    fn play_haptics(
        &mut self,
        index: usize,
        frames: &[HapticFrame],
        audio_haptics: bool,
    ) -> Result<()> {
        let transport = self.selected_transport(index)?;

        for frame in frames {
            let report = build_output_report(
                transport,
                self.next_sequence(),
                None,
                Some((frame.left, frame.right, audio_haptics)),
                LightbarSetup::None,
                None,
            );
            self.send_output_report(index, &report)?;
            thread::sleep(Duration::from_millis(frame.duration_ms));
        }

        let stop = build_output_report(
            transport,
            self.next_sequence(),
            None,
            Some((0, 0, audio_haptics)),
            LightbarSetup::None,
            None,
        );
        self.send_output_report(index, &stop)
    }

    fn set_adaptive_triggers(
        &mut self,
        index: usize,
        left: AdaptiveTriggerEffect,
        right: AdaptiveTriggerEffect,
    ) -> Result<()> {
        let transport = self.selected_transport(index)?;
        let report = build_output_report(
            transport,
            self.next_sequence(),
            None,
            None,
            LightbarSetup::None,
            Some((left, right)),
        );
        self.send_output_report(index, &report)
    }

    fn read_state(&mut self, index: usize) -> Result<Option<GamepadState>> {
        let transport = self.selected_transport(index)?;
        let (report_id, report_len) = match transport {
            TransportKind::Usb => (DS_USB_INPUT_REPORT_ID, DS_USB_INPUT_REPORT_LEN),
            TransportKind::Bluetooth => (DS_BT_INPUT_REPORT_ID, DS_BT_INPUT_REPORT_LEN),
        };
        let mut report = vec![0; report_len];
        report[0] = report_id;
        let mut actual_len = report.len() as CFIndex;
        let result = {
            let device = self.selected_device_mut(index)?;
            device.ensure_open()?;
            unsafe {
                IOHIDDeviceGetReport(
                    device.raw,
                    K_IOHID_REPORT_TYPE_INPUT,
                    CFIndex::from(report_id),
                    report.as_mut_ptr(),
                    &mut actual_len,
                )
            }
        };

        if result == 0 {
            let actual_len = (actual_len as usize).min(report.len());
            if let Some(mut state) = parse_input_report(report_id, &report[..actual_len]) {
                self.input_sequence = self.input_sequence.wrapping_add(1);
                state.packet_count = self.input_sequence;
                Ok(Some(state))
            } else {
                Ok(None)
            }
        } else {
            bail!("IOHIDDeviceGetReport returned {}", format_ioreturn(result))
        }
    }
}

impl Drop for DualSenseBackend {
    fn drop(&mut self) {
        self.devices.clear();
        if !self.manager.is_null() {
            unsafe {
                let _ = IOHIDManagerClose(self.manager, K_IOHID_OPTIONS_TYPE_NONE);
                CFRelease(self.manager as CFTypeRef);
            }
        }
    }
}

struct HidDevice {
    raw: IOHIDDeviceRef,
    opened: bool,
    info: DeviceInfo,
}

impl HidDevice {
    fn new(raw: IOHIDDeviceRef, info: DeviceInfo) -> Self {
        Self {
            raw,
            opened: false,
            info,
        }
    }

    fn ensure_open(&mut self) -> Result<()> {
        if self.opened {
            return Ok(());
        }

        let result = unsafe { IOHIDDeviceOpen(self.raw, K_IOHID_OPTIONS_TYPE_NONE) };
        if result != 0 {
            bail!(
                "IOHIDDeviceOpen for {} returned {}",
                self.info.name,
                format_ioreturn(result)
            );
        }
        self.opened = true;
        Ok(())
    }
}

impl Drop for HidDevice {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                if self.opened {
                    let _ = IOHIDDeviceClose(self.raw, K_IOHID_OPTIONS_TYPE_NONE);
                }
                CFRelease(self.raw as CFTypeRef);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TransportKind {
    Usb,
    Bluetooth,
}

#[derive(Clone, Copy)]
enum LightbarSetup {
    None,
    EnableLightOut,
}

fn build_output_report(
    transport: TransportKind,
    sequence: u8,
    lightbar: Option<Rgb>,
    haptics: Option<(u8, u8, bool)>,
    lightbar_setup: LightbarSetup,
    adaptive_triggers: Option<(AdaptiveTriggerEffect, AdaptiveTriggerEffect)>,
) -> Vec<u8> {
    let (mut report, common_offset) = match transport {
        TransportKind::Usb => {
            let mut report = vec![0; DS_USB_OUTPUT_REPORT_LEN];
            report[0] = DS_USB_OUTPUT_REPORT_ID;
            (report, 1)
        }
        TransportKind::Bluetooth => {
            let mut report = vec![0; DS_BT_OUTPUT_REPORT_LEN];
            report[0] = DS_BT_OUTPUT_REPORT_ID;
            report[1] = (sequence & 0x0f) << 4;
            report[2] = DS_BT_OUTPUT_TAG;
            (report, 3)
        }
    };

    if matches!(lightbar_setup, LightbarSetup::EnableLightOut) {
        apply_lightbar_setup(&mut report, common_offset);
    }

    if let Some(color) = lightbar {
        apply_lightbar(&mut report, common_offset, color);
    }

    if let Some((left, right, audio_haptics)) = haptics {
        apply_haptics(&mut report, common_offset, left, right, audio_haptics);
    }

    if let Some((left, right)) = adaptive_triggers {
        apply_adaptive_triggers(&mut report, common_offset, left, right);
    }

    if matches!(transport, TransportKind::Bluetooth) {
        append_bluetooth_crc(&mut report);
    }

    report
}

fn apply_lightbar_setup(report: &mut [u8], common_offset: usize) {
    report[common_offset + OFFSET_VALID_FLAG2] |=
        DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE;
    report[common_offset + OFFSET_LIGHTBAR_SETUP] = DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_OUT;
}

fn apply_lightbar(report: &mut [u8], common_offset: usize, color: Rgb) {
    report[common_offset + OFFSET_VALID_FLAG1] |= DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE;
    report[common_offset + OFFSET_LIGHTBAR_RED] = color.r;
    report[common_offset + OFFSET_LIGHTBAR_GREEN] = color.g;
    report[common_offset + OFFSET_LIGHTBAR_BLUE] = color.b;
}

fn apply_haptics(
    report: &mut [u8],
    common_offset: usize,
    left: u8,
    right: u8,
    audio_haptics: bool,
) {
    report[common_offset + OFFSET_VALID_FLAG0] |= DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT;
    if audio_haptics {
        report[common_offset + OFFSET_VALID_FLAG2] |= DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2;
    } else {
        report[common_offset + OFFSET_VALID_FLAG0] |= DS_OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION;
    }
    report[common_offset + OFFSET_MOTOR_RIGHT] = right;
    report[common_offset + OFFSET_MOTOR_LEFT] = left;
}

fn apply_adaptive_triggers(
    report: &mut [u8],
    common_offset: usize,
    left: AdaptiveTriggerEffect,
    right: AdaptiveTriggerEffect,
) {
    report[common_offset + OFFSET_VALID_FLAG0] |=
        DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_EFFECT | DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_EFFECT;
    let right_start = common_offset + OFFSET_RIGHT_TRIGGER_EFFECT;
    let left_start = common_offset + OFFSET_LEFT_TRIGGER_EFFECT;
    report[right_start..right_start + right.bytes.len()].copy_from_slice(&right.bytes);
    report[left_start..left_start + left.bytes.len()].copy_from_slice(&left.bytes);
}

fn parse_input_report(report_id: u8, report: &[u8]) -> Option<GamepadState> {
    if matches!(
        report.first().copied(),
        Some(DS_USB_INPUT_REPORT_ID | DS_BT_INPUT_REPORT_ID)
    ) {
        parse_full_input_report(report)
    } else if report_id != 0 {
        let mut full_report = Vec::with_capacity(report.len() + 1);
        full_report.push(report_id);
        full_report.extend_from_slice(report);
        parse_full_input_report(&full_report)
    } else {
        None
    }
}

fn parse_full_input_report(report: &[u8]) -> Option<GamepadState> {
    let common_offset = match report.first().copied()? {
        DS_USB_INPUT_REPORT_ID if report.len() >= DS_USB_INPUT_REPORT_LEN => 1,
        DS_BT_INPUT_REPORT_ID if report.len() >= DS_BT_INPUT_REPORT_LEN => 2,
        _ => return None,
    };
    let minimum_len = common_offset + OFFSET_INPUT_BUTTONS2 + 1;
    if report.len() < minimum_len {
        return None;
    }

    let buttons0 = report[common_offset + OFFSET_INPUT_BUTTONS0];
    let buttons1 = report[common_offset + OFFSET_INPUT_BUTTONS1];
    let buttons2 = report[common_offset + OFFSET_INPUT_BUTTONS2];
    let mut buttons = Vec::new();
    push_dpad_buttons(buttons0 & 0x0f, &mut buttons);
    push_if_pressed(buttons0, 1 << 4, Button::Square, &mut buttons);
    push_if_pressed(buttons0, 1 << 5, Button::Cross, &mut buttons);
    push_if_pressed(buttons0, 1 << 6, Button::Circle, &mut buttons);
    push_if_pressed(buttons0, 1 << 7, Button::Triangle, &mut buttons);
    push_if_pressed(buttons1, 1 << 0, Button::L1, &mut buttons);
    push_if_pressed(buttons1, 1 << 1, Button::R1, &mut buttons);
    push_if_pressed(buttons1, 1 << 2, Button::L2, &mut buttons);
    push_if_pressed(buttons1, 1 << 3, Button::R2, &mut buttons);
    push_if_pressed(buttons1, 1 << 4, Button::Create, &mut buttons);
    push_if_pressed(buttons1, 1 << 5, Button::Options, &mut buttons);
    push_if_pressed(buttons1, 1 << 6, Button::L3, &mut buttons);
    push_if_pressed(buttons1, 1 << 7, Button::R3, &mut buttons);
    push_if_pressed(buttons2, 1 << 0, Button::Ps, &mut buttons);
    push_if_pressed(buttons2, 1 << 1, Button::Touchpad, &mut buttons);
    push_if_pressed(buttons2, 1 << 2, Button::Mute, &mut buttons);

    let battery_percent = report
        .get(common_offset + OFFSET_INPUT_BATTERY)
        .and_then(|value| {
            let capacity = value & 0x0f;
            (capacity <= 10).then_some(capacity.saturating_mul(10))
        });

    Some(GamepadState {
        left_stick: StickState::new(
            report[common_offset + OFFSET_INPUT_LEFT_X],
            report[common_offset + OFFSET_INPUT_LEFT_Y],
        ),
        right_stick: StickState::new(
            report[common_offset + OFFSET_INPUT_RIGHT_X],
            report[common_offset + OFFSET_INPUT_RIGHT_Y],
        ),
        left_trigger: report[common_offset + OFFSET_INPUT_LEFT_TRIGGER],
        right_trigger: report[common_offset + OFFSET_INPUT_RIGHT_TRIGGER],
        buttons,
        battery_percent,
        packet_count: 0,
    })
}

fn push_if_pressed(mask: u8, bit: u8, button: Button, buttons: &mut Vec<Button>) {
    if mask & bit != 0 {
        buttons.push(button);
    }
}

fn push_dpad_buttons(value: u8, buttons: &mut Vec<Button>) {
    match value {
        0 => buttons.push(Button::DpadUp),
        1 => {
            buttons.push(Button::DpadUp);
            buttons.push(Button::DpadRight);
        }
        2 => buttons.push(Button::DpadRight),
        3 => {
            buttons.push(Button::DpadDown);
            buttons.push(Button::DpadRight);
        }
        4 => buttons.push(Button::DpadDown),
        5 => {
            buttons.push(Button::DpadDown);
            buttons.push(Button::DpadLeft);
        }
        6 => buttons.push(Button::DpadLeft),
        7 => {
            buttons.push(Button::DpadUp);
            buttons.push(Button::DpadLeft);
        }
        _ => {}
    }
}

fn append_bluetooth_crc(report: &mut [u8]) {
    let crc_start = report.len() - 4;
    let mut crc = crc32_le(0xffff_ffff, &[DS_BT_CRC_SEED]);
    crc = !crc32_le(crc, &report[..crc_start]);
    report[crc_start..].copy_from_slice(&crc.to_le_bytes());
}

fn crc32_le(mut crc: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn set_vendor_matching(manager: IOHIDManagerRef) -> Result<()> {
    let key = cf_string("VendorID")?;
    let value = cf_number(SONY_VENDOR_ID as i32)?;
    let keys = [key as *const c_void];
    let values = [value as *const c_void];

    let dictionary = unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len() as CFIndex,
            ptr::addr_of!(kCFTypeDictionaryKeyCallBacks),
            ptr::addr_of!(kCFTypeDictionaryValueCallBacks),
        )
    };

    unsafe {
        CFRelease(key as CFTypeRef);
        CFRelease(value as CFTypeRef);
    }

    ensure!(!dictionary.is_null(), "CFDictionaryCreate returned null");
    unsafe {
        IOHIDManagerSetDeviceMatching(manager, dictionary);
        CFRelease(dictionary as CFTypeRef);
    }
    Ok(())
}

fn read_number_property(device: IOHIDDeviceRef, name: &str) -> Option<u32> {
    let key = cf_string(name).ok()?;
    let value = unsafe { IOHIDDeviceGetProperty(device, key) };
    unsafe { CFRelease(key as CFTypeRef) };
    if value.is_null() {
        return None;
    }

    let mut output = 0i32;
    let ok = unsafe {
        CFNumberGetValue(
            value as CFNumberRef,
            K_CF_NUMBER_INT_TYPE,
            &mut output as *mut i32 as *mut c_void,
        )
    };
    (ok != 0).then_some(output as u32)
}

fn read_string_property(device: IOHIDDeviceRef, name: &str) -> Option<String> {
    let key = cf_string(name).ok()?;
    let value = unsafe { IOHIDDeviceGetProperty(device, key) };
    unsafe { CFRelease(key as CFTypeRef) };
    if value.is_null() {
        return None;
    }

    let mut buffer = [0 as c_char; 256];
    let ok = unsafe {
        CFStringGetCString(
            value as CFStringRef,
            buffer.as_mut_ptr(),
            buffer.len() as CFIndex,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if ok == 0 {
        return None;
    }

    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .ok()
        .map(ToOwned::to_owned)
}

fn cf_string(value: &str) -> Result<CFStringRef> {
    let c_string = CString::new(value).with_context(|| format!("invalid CFString {value:?}"))?;
    let cf_string = unsafe {
        CFStringCreateWithCString(ptr::null(), c_string.as_ptr(), K_CF_STRING_ENCODING_UTF8)
    };
    ensure!(
        !cf_string.is_null(),
        "CFStringCreateWithCString returned null"
    );
    Ok(cf_string)
}

fn cf_number(value: i32) -> Result<CFNumberRef> {
    let number = unsafe {
        CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_INT_TYPE,
            &value as *const i32 as *const c_void,
        )
    };
    ensure!(!number.is_null(), "CFNumberCreate returned null");
    Ok(number)
}

fn format_ioreturn(value: IOReturn) -> String {
    format!("0x{:08x}", value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_lightbar_report_uses_common_dualsense_layout() {
        let report = build_output_report(
            TransportKind::Usb,
            0,
            Some(Rgb::new(10, 20, 30)),
            None,
            LightbarSetup::None,
            None,
        );

        assert_eq!(report.len(), DS_USB_OUTPUT_REPORT_LEN);
        assert_eq!(report[0], DS_USB_OUTPUT_REPORT_ID);
        assert_eq!(
            report[1 + OFFSET_VALID_FLAG1] & DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE,
            DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE
        );
        assert_eq!(report[1 + OFFSET_LIGHTBAR_RED], 10);
        assert_eq!(report[1 + OFFSET_LIGHTBAR_GREEN], 20);
        assert_eq!(report[1 + OFFSET_LIGHTBAR_BLUE], 30);
    }

    #[test]
    fn usb_lightbar_setup_report_disables_startup_animation() {
        let report = build_output_report(
            TransportKind::Usb,
            0,
            None,
            None,
            LightbarSetup::EnableLightOut,
            None,
        );

        assert_eq!(
            report[1 + OFFSET_VALID_FLAG2] & DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE,
            DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE
        );
        assert_eq!(
            report[1 + OFFSET_LIGHTBAR_SETUP],
            DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_OUT
        );
    }

    #[test]
    fn bluetooth_report_sets_sequence_tag_and_crc() {
        let report = build_output_report(
            TransportKind::Bluetooth,
            7,
            Some(Rgb::new(1, 2, 3)),
            Some((4, 5, true)),
            LightbarSetup::None,
            None,
        );

        assert_eq!(report.len(), DS_BT_OUTPUT_REPORT_LEN);
        assert_eq!(report[0], DS_BT_OUTPUT_REPORT_ID);
        assert_eq!(report[1], 0x70);
        assert_eq!(report[2], DS_BT_OUTPUT_TAG);
        assert_ne!(&report[DS_BT_OUTPUT_REPORT_LEN - 4..], &[0, 0, 0, 0]);

        let crc_start = report.len() - 4;
        let mut crc = crc32_le(0xffff_ffff, &[DS_BT_CRC_SEED]);
        crc = !crc32_le(crc, &report[..crc_start]);
        assert_eq!(&report[crc_start..], crc.to_le_bytes().as_slice());
    }

    #[test]
    fn audio_haptics_use_vibration_v2_flag() {
        let report = build_output_report(
            TransportKind::Usb,
            0,
            None,
            Some((11, 22, true)),
            LightbarSetup::None,
            None,
        );

        assert_eq!(
            report[1 + OFFSET_VALID_FLAG0] & DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT,
            DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT
        );
        assert_eq!(
            report[1 + OFFSET_VALID_FLAG2] & DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2,
            DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2
        );
        assert_eq!(report[1 + OFFSET_MOTOR_LEFT], 11);
        assert_eq!(report[1 + OFFSET_MOTOR_RIGHT], 22);
    }

    #[test]
    fn adaptive_trigger_report_sets_both_trigger_effect_blocks() {
        let left = AdaptiveTriggerEffect::from_bytes([0x21, 0x04, 0x80, 0, 0, 0, 0, 0, 0, 0, 0]);
        let right =
            AdaptiveTriggerEffect::from_bytes([0x26, 0x02, 0x70, 0x18, 0, 0, 0, 0, 0, 0, 0]);
        let report = build_output_report(
            TransportKind::Usb,
            0,
            None,
            None,
            LightbarSetup::None,
            Some((left, right)),
        );

        assert_eq!(
            report[1 + OFFSET_VALID_FLAG0] & DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_EFFECT,
            DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_EFFECT
        );
        assert_eq!(
            report[1 + OFFSET_VALID_FLAG0] & DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_EFFECT,
            DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_EFFECT
        );
        assert_eq!(
            &report[1 + OFFSET_LEFT_TRIGGER_EFFECT..1 + OFFSET_LEFT_TRIGGER_EFFECT + 11],
            left.bytes.as_slice()
        );
        assert_eq!(
            &report[1 + OFFSET_RIGHT_TRIGGER_EFFECT..1 + OFFSET_RIGHT_TRIGGER_EFFECT + 11],
            right.bytes.as_slice()
        );
    }

    #[test]
    fn usb_input_report_parses_buttons_sticks_and_battery() {
        let mut report = vec![0; DS_USB_INPUT_REPORT_LEN];
        let common = 1;
        report[0] = DS_USB_INPUT_REPORT_ID;
        report[common + OFFSET_INPUT_LEFT_X] = 10;
        report[common + OFFSET_INPUT_LEFT_Y] = 20;
        report[common + OFFSET_INPUT_RIGHT_X] = 30;
        report[common + OFFSET_INPUT_RIGHT_Y] = 40;
        report[common + OFFSET_INPUT_LEFT_TRIGGER] = 50;
        report[common + OFFSET_INPUT_RIGHT_TRIGGER] = 60;
        report[common + OFFSET_INPUT_BUTTONS0] = 8 | (1 << 5);
        report[common + OFFSET_INPUT_BUTTONS1] = (1 << 0) | (1 << 7);
        report[common + OFFSET_INPUT_BUTTONS2] = 1 << 2;
        report[common + OFFSET_INPUT_BATTERY] = 7;

        let state = parse_input_report(DS_USB_INPUT_REPORT_ID, &report).unwrap();

        assert_eq!(state.left_stick, StickState::new(10, 20));
        assert_eq!(state.right_stick, StickState::new(30, 40));
        assert_eq!(state.left_trigger, 50);
        assert_eq!(state.right_trigger, 60);
        assert!(state.is_pressed(Button::Cross));
        assert!(state.is_pressed(Button::L1));
        assert!(state.is_pressed(Button::R3));
        assert!(state.is_pressed(Button::Mute));
        assert!(!state.is_pressed(Button::DpadUp));
        assert_eq!(state.battery_percent, Some(70));
    }

    #[test]
    fn bluetooth_input_report_parses_when_callback_omits_report_id() {
        let mut report = vec![0; DS_BT_INPUT_REPORT_LEN];
        let common = 2;
        report[0] = DS_BT_INPUT_REPORT_ID;
        report[1] = 0x40;
        report[common + OFFSET_INPUT_BUTTONS0] = 1 | (1 << 7);
        report[common + OFFSET_INPUT_BUTTONS1] = 1 << 5;

        let state = parse_input_report(DS_BT_INPUT_REPORT_ID, &report[1..]).unwrap();

        assert!(state.is_pressed(Button::DpadUp));
        assert!(state.is_pressed(Button::DpadRight));
        assert!(state.is_pressed(Button::Triangle));
        assert!(state.is_pressed(Button::Options));
    }
}
