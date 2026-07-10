use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::{
    dualsense::{
        is_supported_dualsense, AdaptiveTriggerEffect, DeviceInfo, DualSenseControl, FirmwareInfo,
        HapticFrame, HapticOutput, SONY_VENDOR_ID,
    },
    model::{
        AudioRoute, BatteryStatus, Button, GamepadState, MotionState, Rgb, StickState,
        SystemProfile, TouchPoint,
    },
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
type CFRunLoopRef = *mut c_void;
type IOOptionBits = u32;
type IOReturn = i32;
type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOHIDReportType = CFIndex;
type IOHIDReportCallback = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        result: IOReturn,
        sender: *mut c_void,
        report_type: IOHIDReportType,
        report_id: u32,
        report: *mut u8,
        report_length: CFIndex,
    ),
>;
type IOHIDDeviceCallback = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        result: IOReturn,
        sender: *mut c_void,
        device: IOHIDDeviceRef,
    ),
>;

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
const K_IOHID_REPORT_TYPE_FEATURE: IOHIDReportType = 2;

const DS_USB_INPUT_REPORT_ID: u8 = 0x01;
const DS_USB_INPUT_REPORT_LEN: usize = 64;
const DS_BT_INPUT_REPORT_ID: u8 = 0x31;
const DS_BT_INPUT_REPORT_LEN: usize = 78;
const DS_USB_OUTPUT_REPORT_ID: u8 = 0x02;
const DS_USB_OUTPUT_REPORT_LEN: usize = 63;
const DS_BT_OUTPUT_REPORT_ID: u8 = 0x31;
const DS_BT_OUTPUT_REPORT_LEN: usize = 78;
const DS_BT_OUTPUT_TAG: u8 = 0x10;
const DS_BT_INPUT_CRC_SEED: u8 = 0xa1;
const DS_BT_CRC_SEED: u8 = 0xa2;
const DS_FEATURE_REPORT_PAIRING_INFO: u8 = 0x09;
const DS_FEATURE_REPORT_PAIRING_INFO_LEN: usize = 20;
const DS_FEATURE_REPORT_FIRMWARE_INFO: u8 = 0x20;
const DS_FEATURE_REPORT_FIRMWARE_INFO_LEN: usize = 64;

const DS_OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION: u8 = 1 << 0;
const DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_EFFECT: u8 = 1 << 2;
const DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_EFFECT: u8 = 1 << 3;
const DS_OUTPUT_VALID_FLAG0_SPEAKER_VOLUME_ENABLE: u8 = 1 << 5;
const DS_OUTPUT_VALID_FLAG0_MIC_VOLUME_ENABLE: u8 = 1 << 6;
const DS_OUTPUT_VALID_FLAG0_AUDIO_CONTROL_ENABLE: u8 = 1 << 7;
const DS_OUTPUT_VALID_FLAG1_MIC_MUTE_LED_CONTROL_ENABLE: u8 = 1 << 0;
const DS_OUTPUT_VALID_FLAG1_POWER_SAVE_CONTROL_ENABLE: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE: u8 = 1 << 2;
const DS_OUTPUT_VALID_FLAG1_PLAYER_INDICATOR_CONTROL_ENABLE: u8 = 1 << 4;
const DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2: u8 = 1 << 2;
const DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_OUT: u8 = 1 << 1;
const DS_OUTPUT_POWER_SAVE_CONTROL_MIC_MUTE: u8 = 1 << 4;
const DS_OUTPUT_AUDIO_ROUTE_HEADPHONES: u8 = 0;
const DS_OUTPUT_AUDIO_ROUTE_SPEAKER: u8 = 0x30;

const OFFSET_VALID_FLAG0: usize = 0;
const OFFSET_VALID_FLAG1: usize = 1;
const OFFSET_MOTOR_RIGHT: usize = 2;
const OFFSET_MOTOR_LEFT: usize = 3;
const OFFSET_SPEAKER_VOLUME: usize = 5;
const OFFSET_MIC_VOLUME: usize = 6;
const OFFSET_AUDIO_CONTROL: usize = 7;
const OFFSET_MUTE_BUTTON_LED: usize = 8;
const OFFSET_POWER_SAVE_CONTROL: usize = 9;
const OFFSET_RIGHT_TRIGGER_EFFECT: usize = 10;
const OFFSET_LEFT_TRIGGER_EFFECT: usize = 21;
const OFFSET_VALID_FLAG2: usize = 38;
const OFFSET_LIGHTBAR_SETUP: usize = 41;
const OFFSET_PLAYER_LEDS: usize = 43;
const OFFSET_LIGHTBAR_RED: usize = 44;
const OFFSET_LIGHTBAR_GREEN: usize = 45;
const OFFSET_LIGHTBAR_BLUE: usize = 46;

const OFFSET_INPUT_LEFT_X: usize = 0;
const OFFSET_INPUT_LEFT_Y: usize = 1;
const OFFSET_INPUT_RIGHT_X: usize = 2;
const OFFSET_INPUT_RIGHT_Y: usize = 3;
const OFFSET_INPUT_LEFT_TRIGGER: usize = 4;
const OFFSET_INPUT_RIGHT_TRIGGER: usize = 5;
const OFFSET_INPUT_SEQUENCE: usize = 6;
const OFFSET_INPUT_BUTTONS0: usize = 7;
const OFFSET_INPUT_BUTTONS1: usize = 8;
const OFFSET_INPUT_BUTTONS2: usize = 9;
const OFFSET_INPUT_GYRO: usize = 15;
const OFFSET_INPUT_ACCEL: usize = 21;
const OFFSET_INPUT_SENSOR_TIMESTAMP: usize = 27;
const OFFSET_INPUT_TOUCH_0: usize = 32;
const OFFSET_INPUT_TOUCH_1: usize = 36;
const OFFSET_INPUT_BATTERY: usize = 52;
const OFFSET_INPUT_STATUS1: usize = 53;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
    static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;
    static kCFRunLoopDefaultMode: CFStringRef;

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
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    fn CFSetGetCount(the_set: CFSetRef) -> CFIndex;
    fn CFSetGetValues(the_set: CFSetRef, values: *mut *const c_void);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRunInMode(
        mode: CFStringRef,
        seconds: f64,
        return_after_source_handled: Boolean,
    ) -> c_int;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: CFOptionFlags) -> IOHIDManagerRef;
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: IOOptionBits) -> IOReturn;
    fn IOHIDManagerClose(manager: IOHIDManagerRef, options: IOOptionBits) -> IOReturn;
    fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;
    fn IOHIDManagerRegisterInputReportCallback(
        manager: IOHIDManagerRef,
        callback: IOHIDReportCallback,
        context: *mut c_void,
    );
    fn IOHIDManagerRegisterDeviceMatchingCallback(
        manager: IOHIDManagerRef,
        callback: IOHIDDeviceCallback,
        context: *mut c_void,
    );
    fn IOHIDManagerRegisterDeviceRemovalCallback(
        manager: IOHIDManagerRef,
        callback: IOHIDDeviceCallback,
        context: *mut c_void,
    );
    fn IOHIDManagerScheduleWithRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFStringRef,
    );
    fn IOHIDManagerUnscheduleFromRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFStringRef,
    );
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
    run_loop: CFRunLoopRef,
    input_state: Box<InputReportState>,
    device_change_state: Box<DeviceChangeState>,
    devices: Vec<HidDevice>,
    sequence: u8,
}

impl DualSenseBackend {
    pub fn new() -> Result<Self> {
        let manager = unsafe { IOHIDManagerCreate(ptr::null(), K_IOHID_OPTIONS_TYPE_NONE) };
        ensure!(!manager.is_null(), "IOHIDManagerCreate returned null");

        if let Err(error) =
            set_vendor_matching(manager).context("failed to install IOKit matching dictionary")
        {
            unsafe { CFRelease(manager as CFTypeRef) };
            return Err(error);
        }

        let run_loop = unsafe { CFRunLoopGetCurrent() };
        if run_loop.is_null() {
            unsafe { CFRelease(manager as CFTypeRef) };
            bail!("CFRunLoopGetCurrent returned null");
        }

        let mut input_state = Box::<InputReportState>::default();
        let input_context = (&mut *input_state as *mut InputReportState).cast::<c_void>();
        let mut device_change_state = Box::<DeviceChangeState>::default();
        let device_change_context =
            (&mut *device_change_state as *mut DeviceChangeState).cast::<c_void>();
        unsafe {
            IOHIDManagerRegisterInputReportCallback(
                manager,
                Some(input_report_callback),
                input_context,
            );
            IOHIDManagerRegisterDeviceMatchingCallback(
                manager,
                Some(device_change_callback),
                device_change_context,
            );
            IOHIDManagerRegisterDeviceRemovalCallback(
                manager,
                Some(device_change_callback),
                device_change_context,
            );
            IOHIDManagerScheduleWithRunLoop(manager, run_loop, kCFRunLoopDefaultMode);
        }

        let open_result = unsafe { IOHIDManagerOpen(manager, K_IOHID_OPTIONS_TYPE_NONE) };
        if open_result != 0 {
            unsafe {
                IOHIDManagerRegisterInputReportCallback(manager, None, ptr::null_mut());
                IOHIDManagerRegisterDeviceMatchingCallback(manager, None, ptr::null_mut());
                IOHIDManagerRegisterDeviceRemovalCallback(manager, None, ptr::null_mut());
                IOHIDManagerUnscheduleFromRunLoop(manager, run_loop, kCFRunLoopDefaultMode);
                CFRelease(manager as CFTypeRef);
            }
            bail!("IOHIDManagerOpen returned {}", format_ioreturn(open_result));
        }

        let mut backend = Self {
            manager,
            run_loop,
            input_state,
            device_change_state,
            devices: Vec::new(),
            sequence: 0,
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

    fn send_lightbar_color_only(&mut self, index: usize, color: Rgb) -> Result<()> {
        let transport = self.selected_transport(index)?;
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
}

impl DualSenseControl for DualSenseBackend {
    fn refresh(&mut self) -> Result<()> {
        self.devices.clear();
        self.input_state.clear();

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
                mac_address: None,
                firmware: None,
                diagnostics_error: None,
            };
            let is_edge = info.is_edge();
            let mut device = HidDevice::new(retained, info);
            device.refresh_diagnostics();
            self.input_state.register_device(retained, is_edge);
            self.devices.push(device);
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
        let needs_setup = !self.devices[index].lightbar_setup_completed;

        if needs_setup {
            // Firmware setup deliberately fades the lightbar out. It is only
            // needed after a physical connection, never for focus reapply.
            let setup = build_output_report(
                transport,
                self.next_sequence(),
                None,
                None,
                LightbarSetup::EnableLightOut,
                None,
            );
            self.send_output_report(index, &setup)?;
            self.devices[index].lightbar_setup_completed = true;
        }

        self.send_lightbar_color_only(index, color)
    }

    fn reapply_lightbar(&mut self, index: usize, color: Rgb) -> Result<()> {
        self.send_lightbar_color_only(index, color)
    }

    fn pulse_haptics(
        &mut self,
        index: usize,
        left: u8,
        right: u8,
        audio_haptics: bool,
    ) -> Result<()> {
        self.play_haptics(
            index,
            &[HapticFrame::new(left, right, 180).symmetrized()],
            audio_haptics,
        )
    }

    fn set_haptics(
        &mut self,
        index: usize,
        output: HapticOutput,
        audio_haptics: bool,
    ) -> Result<()> {
        let output = output.symmetrized();
        debug_assert!(output.is_symmetric());
        let transport = self.selected_transport(index)?;
        let report = build_output_report(
            transport,
            self.next_sequence(),
            None,
            Some((output.heavy, output.sharp, audio_haptics)),
            LightbarSetup::None,
            None,
        );
        self.send_output_report(index, &report)
    }

    fn play_haptics(
        &mut self,
        index: usize,
        frames: &[HapticFrame],
        audio_haptics: bool,
    ) -> Result<()> {
        for frame in frames {
            let frame = frame.symmetrized();
            debug_assert!(frame.is_symmetric());
            self.set_haptics(
                index,
                HapticOutput::new(frame.left, frame.right),
                audio_haptics,
            )?;
            thread::sleep(Duration::from_millis(frame.duration_ms));
        }

        self.set_haptics(index, HapticOutput::OFF, audio_haptics)
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

    fn set_system_controls(&mut self, index: usize, system: &SystemProfile) -> Result<()> {
        let transport = self.selected_transport(index)?;
        let mut report = build_output_report(
            transport,
            self.next_sequence(),
            None,
            None,
            LightbarSetup::None,
            None,
        );
        apply_system_controls(&mut report, output_common_offset(transport), system);
        if matches!(transport, TransportKind::Bluetooth) {
            append_bluetooth_crc(&mut report);
        }
        self.send_output_report(index, &report)
    }

    fn pump_events(&mut self) -> Result<()> {
        ensure!(!self.run_loop.is_null(), "input run loop is unavailable");
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
        }
        Ok(())
    }

    fn take_device_change(&mut self) -> bool {
        self.device_change_state.take()
    }

    fn read_state(&mut self, index: usize) -> Result<Option<GamepadState>> {
        let device_key = self
            .devices
            .get(index)
            .map(|device| device.raw as usize)
            .ok_or_else(|| anyhow!("no DualSense device selected"))?;
        Ok(self.input_state.latest_for(device_key))
    }
}

impl Drop for DualSenseBackend {
    fn drop(&mut self) {
        self.devices.clear();
        if !self.manager.is_null() {
            unsafe {
                IOHIDManagerRegisterInputReportCallback(self.manager, None, ptr::null_mut());
                IOHIDManagerRegisterDeviceMatchingCallback(self.manager, None, ptr::null_mut());
                IOHIDManagerRegisterDeviceRemovalCallback(self.manager, None, ptr::null_mut());
                if !self.run_loop.is_null() {
                    IOHIDManagerUnscheduleFromRunLoop(
                        self.manager,
                        self.run_loop,
                        kCFRunLoopDefaultMode,
                    );
                }
                let _ = IOHIDManagerClose(self.manager, K_IOHID_OPTIONS_TYPE_NONE);
                CFRelease(self.manager as CFTypeRef);
            }
        }
    }
}

#[derive(Default)]
struct DeviceChangeState {
    changed: AtomicBool,
}

impl DeviceChangeState {
    fn mark_changed(&self) {
        self.changed.store(true, Ordering::Release);
    }

    fn take(&self) -> bool {
        self.changed.swap(false, Ordering::AcqRel)
    }
}

#[derive(Default)]
struct InputReportState {
    reports: Mutex<HashMap<usize, InputReportSlot>>,
}

struct InputReportSlot {
    is_edge: bool,
    latest_state: Option<GamepadState>,
    packet_count: u64,
}

impl InputReportState {
    fn clear(&self) {
        if let Ok(mut reports) = self.reports.lock() {
            reports.clear();
        }
    }

    fn register_device(&self, raw: IOHIDDeviceRef, is_edge: bool) {
        if raw.is_null() {
            return;
        }

        self.register(raw as usize, is_edge);
    }

    fn register(&self, device_key: usize, is_edge: bool) {
        if let Ok(mut reports) = self.reports.lock() {
            reports.insert(
                device_key,
                InputReportSlot {
                    is_edge,
                    latest_state: None,
                    packet_count: 0,
                },
            );
        }
    }

    fn record(&self, device_key: usize, report_id: u8, report: &[u8]) {
        let Ok(mut reports) = self.reports.lock() else {
            return;
        };
        let Some(slot) = reports.get_mut(&device_key) else {
            return;
        };
        let Some(mut state) = parse_input_report(report_id, report, slot.is_edge) else {
            return;
        };

        slot.packet_count = slot.packet_count.wrapping_add(1);
        state.packet_count = slot.packet_count;
        slot.latest_state = Some(state);
    }

    fn latest_for(&self, device_key: usize) -> Option<GamepadState> {
        self.reports
            .lock()
            .ok()?
            .get(&device_key)
            .and_then(|slot| slot.latest_state.clone())
    }
}

unsafe extern "C" fn input_report_callback(
    context: *mut c_void,
    result: IOReturn,
    sender: *mut c_void,
    report_type: IOHIDReportType,
    report_id: u32,
    report: *mut u8,
    report_length: CFIndex,
) {
    if result != 0
        || sender.is_null()
        || report.is_null()
        || report_length <= 0
        || report_type != K_IOHID_REPORT_TYPE_INPUT
        || report_id > u32::from(u8::MAX)
    {
        return;
    }

    // `context` comes from the boxed state held by `DualSenseBackend` until after unregistering.
    let Some(input_state) = (unsafe { (context as *const InputReportState).as_ref() }) else {
        return;
    };
    let Ok(report_length) = usize::try_from(report_length) else {
        return;
    };
    let report = unsafe { std::slice::from_raw_parts(report, report_length) };
    input_state.record(sender as usize, report_id as u8, report);
}

unsafe extern "C" fn device_change_callback(
    context: *mut c_void,
    result: IOReturn,
    _sender: *mut c_void,
    device: IOHIDDeviceRef,
) {
    if result != 0 || device.is_null() {
        return;
    }

    // `context` comes from the boxed state held by `DualSenseBackend` until after unregistering.
    let Some(device_change_state) = (unsafe { (context as *const DeviceChangeState).as_ref() })
    else {
        return;
    };
    device_change_state.mark_changed();
}

struct HidDevice {
    raw: IOHIDDeviceRef,
    opened: bool,
    lightbar_setup_completed: bool,
    info: DeviceInfo,
}

impl HidDevice {
    fn new(raw: IOHIDDeviceRef, info: DeviceInfo) -> Self {
        Self {
            raw,
            opened: false,
            lightbar_setup_completed: false,
            info,
        }
    }

    fn ensure_open(&mut self) -> Result<()> {
        if !self.opened {
            let result = unsafe { IOHIDDeviceOpen(self.raw, K_IOHID_OPTIONS_TYPE_NONE) };
            if result != 0 {
                bail!(
                    "IOHIDDeviceOpen for {} returned {}",
                    self.info.name,
                    format_ioreturn(result)
                );
            }
            self.opened = true;
        }

        Ok(())
    }

    fn refresh_diagnostics(&mut self) {
        let mut errors = Vec::new();
        if let Err(error) = self.ensure_open() {
            self.info.diagnostics_error = Some(format!("Could not open HID device: {error:#}"));
            return;
        }

        match self.read_feature_report(
            DS_FEATURE_REPORT_FIRMWARE_INFO,
            DS_FEATURE_REPORT_FIRMWARE_INFO_LEN,
        ) {
            Ok(report) => match parse_firmware_info(&report) {
                Some(firmware) => self.info.firmware = Some(firmware),
                None => errors.push("firmware report has an unexpected layout".to_string()),
            },
            Err(error) => errors.push(format!("firmware report unavailable: {error:#}")),
        }

        match self.read_feature_report(
            DS_FEATURE_REPORT_PAIRING_INFO,
            DS_FEATURE_REPORT_PAIRING_INFO_LEN,
        ) {
            Ok(report) => match parse_pairing_mac_address(&report) {
                Some(address) => self.info.mac_address = Some(address),
                None => errors.push("pairing report has an unexpected layout".to_string()),
            },
            Err(error) => errors.push(format!("pairing report unavailable: {error:#}")),
        }

        self.info.diagnostics_error = (!errors.is_empty()).then(|| errors.join("; "));
    }

    fn read_feature_report(&mut self, report_id: u8, expected_len: usize) -> Result<Vec<u8>> {
        let mut report = vec![0; expected_len];
        report[0] = report_id;
        let mut actual_len = expected_len as CFIndex;
        let result = unsafe {
            IOHIDDeviceGetReport(
                self.raw,
                K_IOHID_REPORT_TYPE_FEATURE,
                CFIndex::from(report_id),
                report.as_mut_ptr(),
                &mut actual_len,
            )
        };
        if result != 0 {
            bail!(
                "IOHIDDeviceGetReport feature 0x{report_id:02x} returned {}",
                format_ioreturn(result)
            );
        }

        let actual_len = usize::try_from(actual_len).unwrap_or(0).min(report.len());
        ensure!(actual_len > 0, "feature 0x{report_id:02x} was empty");
        report.truncate(actual_len);
        Ok(report)
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

const fn output_common_offset(transport: TransportKind) -> usize {
    match transport {
        TransportKind::Usb => 1,
        TransportKind::Bluetooth => 3,
    }
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
    let output = HapticOutput::new(left, right).symmetrized();
    report[common_offset + OFFSET_VALID_FLAG0] |= DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT;
    if audio_haptics {
        report[common_offset + OFFSET_VALID_FLAG2] |= DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2;
    } else {
        report[common_offset + OFFSET_VALID_FLAG0] |= DS_OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION;
    }
    report[common_offset + OFFSET_MOTOR_RIGHT] = output.sharp;
    report[common_offset + OFFSET_MOTOR_LEFT] = output.heavy;
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

fn apply_system_controls(report: &mut [u8], common_offset: usize, system: &SystemProfile) {
    report[common_offset + OFFSET_VALID_FLAG1] |=
        DS_OUTPUT_VALID_FLAG1_PLAYER_INDICATOR_CONTROL_ENABLE
            | DS_OUTPUT_VALID_FLAG1_MIC_MUTE_LED_CONTROL_ENABLE
            | DS_OUTPUT_VALID_FLAG1_POWER_SAVE_CONTROL_ENABLE;
    report[common_offset + OFFSET_PLAYER_LEDS] = system.player_indicator.led_mask();
    report[common_offset + OFFSET_MUTE_BUTTON_LED] = u8::from(system.microphone_muted);
    report[common_offset + OFFSET_POWER_SAVE_CONTROL] = if system.microphone_muted {
        DS_OUTPUT_POWER_SAVE_CONTROL_MIC_MUTE
    } else {
        0
    };

    report[common_offset + OFFSET_VALID_FLAG0] |=
        DS_OUTPUT_VALID_FLAG0_SPEAKER_VOLUME_ENABLE | DS_OUTPUT_VALID_FLAG0_MIC_VOLUME_ENABLE;
    report[common_offset + OFFSET_SPEAKER_VOLUME] = system.speaker_volume;
    report[common_offset + OFFSET_MIC_VOLUME] = system.microphone_volume.min(0x40);

    let audio_route = match system.audio_route {
        AudioRoute::Unchanged => None,
        AudioRoute::Headphones => Some(DS_OUTPUT_AUDIO_ROUTE_HEADPHONES),
        AudioRoute::Speaker => Some(DS_OUTPUT_AUDIO_ROUTE_SPEAKER),
    };
    if let Some(audio_route) = audio_route {
        report[common_offset + OFFSET_VALID_FLAG0] |= DS_OUTPUT_VALID_FLAG0_AUDIO_CONTROL_ENABLE;
        report[common_offset + OFFSET_AUDIO_CONTROL] = audio_route;
    }
}

fn parse_input_report(report_id: u8, report: &[u8], is_edge: bool) -> Option<GamepadState> {
    let expected_full_len = match report_id {
        DS_USB_INPUT_REPORT_ID => DS_USB_INPUT_REPORT_LEN,
        DS_BT_INPUT_REPORT_ID => DS_BT_INPUT_REPORT_LEN,
        _ => return None,
    };
    if report.len() >= expected_full_len && report.first().copied() == Some(report_id) {
        parse_full_input_report(report, is_edge)
    } else {
        let mut full_report = Vec::with_capacity(report.len() + 1);
        full_report.push(report_id);
        full_report.extend_from_slice(report);
        parse_full_input_report(&full_report, is_edge)
    }
}

fn parse_full_input_report(report: &[u8], is_edge: bool) -> Option<GamepadState> {
    let common_offset = match report.first().copied()? {
        DS_USB_INPUT_REPORT_ID if report.len() >= DS_USB_INPUT_REPORT_LEN => 1,
        DS_BT_INPUT_REPORT_ID if report.len() >= DS_BT_INPUT_REPORT_LEN => 2,
        _ => return None,
    };
    let minimum_len = common_offset + OFFSET_INPUT_STATUS1 + 1;
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
    if is_edge {
        push_if_pressed(buttons2, 1 << 4, Button::Fn1, &mut buttons);
        push_if_pressed(buttons2, 1 << 5, Button::Fn2, &mut buttons);
        push_if_pressed(buttons2, 1 << 6, Button::LeftPaddle, &mut buttons);
        push_if_pressed(buttons2, 1 << 7, Button::RightPaddle, &mut buttons);
    }

    let status0 = report[common_offset + OFFSET_INPUT_BATTERY];
    let status1 = report[common_offset + OFFSET_INPUT_STATUS1];
    let (battery_percent, battery_status) = parse_battery_status(status0);

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
        battery_status,
        headset_connected: status1 & (1 << 0) != 0,
        microphone_connected: status1 & (1 << 1) != 0,
        microphone_muted: status1 & (1 << 2) != 0,
        touch_points: [
            parse_touch_point(report, common_offset + OFFSET_INPUT_TOUCH_0),
            parse_touch_point(report, common_offset + OFFSET_INPUT_TOUCH_1),
        ],
        motion: MotionState {
            gyro: [
                read_i16_le(report, common_offset + OFFSET_INPUT_GYRO)?,
                read_i16_le(report, common_offset + OFFSET_INPUT_GYRO + 2)?,
                read_i16_le(report, common_offset + OFFSET_INPUT_GYRO + 4)?,
            ],
            accel: [
                read_i16_le(report, common_offset + OFFSET_INPUT_ACCEL)?,
                read_i16_le(report, common_offset + OFFSET_INPUT_ACCEL + 2)?,
                read_i16_le(report, common_offset + OFFSET_INPUT_ACCEL + 4)?,
            ],
            sensor_timestamp: read_u32_le(report, common_offset + OFFSET_INPUT_SENSOR_TIMESTAMP)?,
        },
        report_sequence: report[common_offset + OFFSET_INPUT_SEQUENCE],
        bluetooth_crc_valid: bluetooth_input_crc_valid(report),
        packet_count: 0,
    })
}

fn parse_battery_status(status0: u8) -> (Option<u8>, BatteryStatus) {
    let battery_data = status0 & 0x0f;
    match status0 >> 4 {
        0x0 => (
            Some(battery_data.saturating_mul(10).saturating_add(5).min(100)),
            BatteryStatus::Discharging,
        ),
        0x1 => (
            Some(battery_data.saturating_mul(10).saturating_add(5).min(100)),
            BatteryStatus::Charging,
        ),
        0x2 => (Some(100), BatteryStatus::Full),
        0xa | 0xb => (Some(0), BatteryStatus::Error),
        _ => (None, BatteryStatus::Unknown),
    }
}

fn parse_touch_point(report: &[u8], offset: usize) -> Option<TouchPoint> {
    let contact = *report.get(offset)?;
    if contact & 0x80 != 0 {
        return None;
    }

    let x_low = *report.get(offset + 1)?;
    let packed = *report.get(offset + 2)?;
    let y_high = *report.get(offset + 3)?;
    Some(TouchPoint {
        contact_id: contact & 0x7f,
        x: u16::from(x_low) | (u16::from(packed & 0x0f) << 8),
        y: u16::from(packed >> 4) | (u16::from(y_high) << 4),
    })
}

fn read_i16_le(report: &[u8], offset: usize) -> Option<i16> {
    let bytes = report.get(offset..offset + 2)?;
    Some(i16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u16_le(report: &[u8], offset: usize) -> Option<u16> {
    let bytes = report.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(report: &[u8], offset: usize) -> Option<u32> {
    let bytes = report.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn bluetooth_input_crc_valid(report: &[u8]) -> Option<bool> {
    if report.first().copied() != Some(DS_BT_INPUT_REPORT_ID)
        || report.len() != DS_BT_INPUT_REPORT_LEN
    {
        return None;
    }

    let crc_offset = report.len() - 4;
    let expected = read_u32_le(report, crc_offset)?;
    if expected == 0 {
        return None;
    }

    let mut crc = crc32_le(0xffff_ffff, &[DS_BT_INPUT_CRC_SEED]);
    crc = !crc32_le(crc, &report[..crc_offset]);
    Some(crc == expected)
}

fn parse_firmware_info(report: &[u8]) -> Option<FirmwareInfo> {
    let without_report_id = usize::from(
        report.len() < DS_FEATURE_REPORT_FIRMWARE_INFO_LEN
            || report.first().copied() != Some(DS_FEATURE_REPORT_FIRMWARE_INFO),
    );
    Some(FirmwareInfo {
        hardware_version: read_u32_le(report, 24usize.checked_sub(without_report_id)?)?,
        firmware_version: read_u32_le(report, 28usize.checked_sub(without_report_id)?)?,
        feature_version: read_u16_le(report, 44usize.checked_sub(without_report_id)?)?,
    })
}

fn parse_pairing_mac_address(report: &[u8]) -> Option<String> {
    let without_report_id = usize::from(
        report.len() < DS_FEATURE_REPORT_PAIRING_INFO_LEN
            || report.first().copied() != Some(DS_FEATURE_REPORT_PAIRING_INFO),
    );
    let start = 1usize.checked_sub(without_report_id)?;
    let bytes = report.get(start..start + 6)?;
    Some(
        bytes
            .iter()
            .rev()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
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
            ptr::addr_of!(kCFTypeDictionaryKeyCallBacks).cast(),
            ptr::addr_of!(kCFTypeDictionaryValueCallBacks).cast(),
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
    fn device_change_notification_is_consumed_once() {
        let state = DeviceChangeState::default();

        assert!(!state.take());
        state.mark_changed();
        state.mark_changed();
        assert!(state.take());
        assert!(!state.take());
    }

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
        assert_eq!(
            report[1 + OFFSET_VALID_FLAG2] & DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE,
            0
        );
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
            Some((17, 17, true)),
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
        assert_eq!(report[1 + OFFSET_MOTOR_LEFT], 17);
        assert_eq!(report[1 + OFFSET_MOTOR_RIGHT], 17);
    }

    #[test]
    fn haptics_reports_mirror_legacy_channel_values_for_every_transport() {
        let requested = HapticOutput::new(0x12, 0xe7);
        let expected = requested.symmetrized();

        for (transport, common_offset) in [(TransportKind::Usb, 1), (TransportKind::Bluetooth, 3)] {
            for audio_haptics in [false, true] {
                let report = build_output_report(
                    transport,
                    0,
                    None,
                    Some((requested.heavy, requested.sharp, audio_haptics)),
                    LightbarSetup::None,
                    None,
                );

                assert_eq!(report[common_offset + OFFSET_MOTOR_LEFT], expected.heavy);
                assert_eq!(report[common_offset + OFFSET_MOTOR_RIGHT], expected.sharp);
                assert_eq!(
                    report[common_offset + OFFSET_MOTOR_LEFT],
                    report[common_offset + OFFSET_MOTOR_RIGHT]
                );
                if audio_haptics {
                    assert_ne!(
                        report[common_offset + OFFSET_VALID_FLAG2]
                            & DS_OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2,
                        0
                    );
                } else {
                    assert_ne!(
                        report[common_offset + OFFSET_VALID_FLAG0]
                            & DS_OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION,
                        0
                    );
                }
            }
        }
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

        let state = parse_input_report(DS_USB_INPUT_REPORT_ID, &report, false).unwrap();

        assert_eq!(state.left_stick, StickState::new(10, 20));
        assert_eq!(state.right_stick, StickState::new(30, 40));
        assert_eq!(state.left_trigger, 50);
        assert_eq!(state.right_trigger, 60);
        assert!(state.is_pressed(Button::Cross));
        assert!(state.is_pressed(Button::L1));
        assert!(state.is_pressed(Button::R3));
        assert!(state.is_pressed(Button::Mute));
        assert!(!state.is_pressed(Button::DpadUp));
        assert_eq!(state.battery_percent, Some(75));
        assert_eq!(state.battery_status, BatteryStatus::Discharging);
    }

    #[test]
    fn bluetooth_input_report_parses_when_callback_omits_report_id() {
        let mut report = [0; DS_BT_INPUT_REPORT_LEN];
        let common = 2;
        report[0] = DS_BT_INPUT_REPORT_ID;
        report[1] = 0x40;
        report[common + OFFSET_INPUT_BUTTONS0] = 1 | (1 << 7);
        report[common + OFFSET_INPUT_BUTTONS1] = 1 << 5;

        let state = parse_input_report(DS_BT_INPUT_REPORT_ID, &report[1..], false).unwrap();

        assert!(state.is_pressed(Button::DpadUp));
        assert!(state.is_pressed(Button::DpadRight));
        assert!(state.is_pressed(Button::Triangle));
        assert!(state.is_pressed(Button::Options));
    }

    #[test]
    fn manager_input_state_caches_callback_reports() {
        let input_state = InputReportState::default();
        let device_key = 42;
        input_state.register(device_key, false);

        let mut report = [0; DS_USB_INPUT_REPORT_LEN - 1];
        report[OFFSET_INPUT_LEFT_X] = 17;
        input_state.record(device_key, DS_USB_INPUT_REPORT_ID, &report);
        input_state.record(device_key, DS_USB_INPUT_REPORT_ID, &report);

        let state = input_state.latest_for(device_key).unwrap();
        assert_eq!(state.left_stick.x, 17);
        assert_eq!(state.packet_count, 2);
    }

    #[test]
    fn payload_starting_with_report_id_is_not_mistaken_for_a_full_report() {
        let mut report = [0; DS_USB_INPUT_REPORT_LEN];
        let common = 1;
        report[0] = DS_USB_INPUT_REPORT_ID;
        report[common + OFFSET_INPUT_LEFT_X] = DS_USB_INPUT_REPORT_ID;
        report[common + OFFSET_INPUT_RIGHT_X] = 99;

        let state = parse_input_report(DS_USB_INPUT_REPORT_ID, &report[1..], false).unwrap();

        assert_eq!(state.left_stick.x, DS_USB_INPUT_REPORT_ID);
        assert_eq!(state.right_stick.x, 99);
    }

    #[test]
    fn extended_input_report_parses_touch_motion_status_and_edge_buttons() {
        let mut report = [0; DS_USB_INPUT_REPORT_LEN];
        let common = 1;
        report[0] = DS_USB_INPUT_REPORT_ID;
        report[common + OFFSET_INPUT_BUTTONS2] = 0b1111_0000;
        report[common + OFFSET_INPUT_BATTERY] = 0x19;
        report[common + OFFSET_INPUT_STATUS1] = 0b0000_0111;
        report[common + OFFSET_INPUT_SEQUENCE] = 42;

        report[common + OFFSET_INPUT_GYRO..common + OFFSET_INPUT_GYRO + 2]
            .copy_from_slice(&(-120i16).to_le_bytes());
        report[common + OFFSET_INPUT_GYRO + 2..common + OFFSET_INPUT_GYRO + 4]
            .copy_from_slice(&(240i16).to_le_bytes());
        report[common + OFFSET_INPUT_GYRO + 4..common + OFFSET_INPUT_GYRO + 6]
            .copy_from_slice(&(360i16).to_le_bytes());
        report[common + OFFSET_INPUT_ACCEL..common + OFFSET_INPUT_ACCEL + 2]
            .copy_from_slice(&(8192i16).to_le_bytes());
        report[common + OFFSET_INPUT_ACCEL + 2..common + OFFSET_INPUT_ACCEL + 4]
            .copy_from_slice(&(-8192i16).to_le_bytes());
        report[common + OFFSET_INPUT_ACCEL + 4..common + OFFSET_INPUT_ACCEL + 6]
            .copy_from_slice(&(123i16).to_le_bytes());
        report[common + OFFSET_INPUT_SENSOR_TIMESTAMP..common + OFFSET_INPUT_SENSOR_TIMESTAMP + 4]
            .copy_from_slice(&(0x1234_5678u32).to_le_bytes());

        let touch = common + OFFSET_INPUT_TOUCH_0;
        report[touch] = 3;
        report[touch + 1] = 0xd2;
        report[touch + 2] = 0xb4;
        report[touch + 3] = 0x3d;
        report[common + OFFSET_INPUT_TOUCH_1] = 0x80;

        let state = parse_input_report(DS_USB_INPUT_REPORT_ID, &report, true).unwrap();

        assert!(state.is_pressed(Button::Fn1));
        assert!(state.is_pressed(Button::Fn2));
        assert!(state.is_pressed(Button::LeftPaddle));
        assert!(state.is_pressed(Button::RightPaddle));
        assert_eq!(state.battery_percent, Some(95));
        assert_eq!(state.battery_status, BatteryStatus::Charging);
        assert!(state.headset_connected);
        assert!(state.microphone_connected);
        assert!(state.microphone_muted);
        assert_eq!(
            state.touch_points[0],
            Some(TouchPoint {
                contact_id: 3,
                x: 1234,
                y: 987,
            })
        );
        assert_eq!(state.touch_points[1], None);
        assert_eq!(state.motion.gyro, [-120, 240, 360]);
        assert_eq!(state.motion.accel, [8192, -8192, 123]);
        assert_eq!(state.motion.sensor_timestamp, 0x1234_5678);
        assert_eq!(state.report_sequence, 42);
    }

    #[test]
    fn bluetooth_input_crc_is_reported_without_rejecting_a_frame() {
        let mut report = [0; DS_BT_INPUT_REPORT_LEN];
        report[0] = DS_BT_INPUT_REPORT_ID;
        let crc_offset = report.len() - 4;
        let mut crc = crc32_le(0xffff_ffff, &[DS_BT_INPUT_CRC_SEED]);
        crc = !crc32_le(crc, &report[..crc_offset]);
        report[crc_offset..].copy_from_slice(&crc.to_le_bytes());

        let valid = parse_input_report(DS_BT_INPUT_REPORT_ID, &report, false).unwrap();
        assert_eq!(valid.bluetooth_crc_valid, Some(true));

        report[crc_offset] ^= 0xff;
        let invalid = parse_input_report(DS_BT_INPUT_REPORT_ID, &report, false).unwrap();
        assert_eq!(invalid.bluetooth_crc_valid, Some(false));
    }

    #[test]
    fn system_controls_use_documented_output_offsets_and_flags() {
        let system = SystemProfile {
            player_indicator: crate::model::PlayerIndicator::Player3,
            microphone_muted: true,
            speaker_volume: 180,
            microphone_volume: 255,
            audio_route: AudioRoute::Speaker,
        };

        let mut report =
            build_output_report(TransportKind::Usb, 0, None, None, LightbarSetup::None, None);
        apply_system_controls(
            &mut report,
            output_common_offset(TransportKind::Usb),
            &system,
        );

        let common = 1;
        assert_eq!(
            report[common + OFFSET_VALID_FLAG1]
                & DS_OUTPUT_VALID_FLAG1_PLAYER_INDICATOR_CONTROL_ENABLE,
            DS_OUTPUT_VALID_FLAG1_PLAYER_INDICATOR_CONTROL_ENABLE
        );
        assert_eq!(report[common + OFFSET_PLAYER_LEDS], 0b10101);
        assert_eq!(report[common + OFFSET_MUTE_BUTTON_LED], 1);
        assert_eq!(
            report[common + OFFSET_POWER_SAVE_CONTROL],
            DS_OUTPUT_POWER_SAVE_CONTROL_MIC_MUTE
        );
        assert_eq!(report[common + OFFSET_SPEAKER_VOLUME], 180);
        assert_eq!(report[common + OFFSET_MIC_VOLUME], 0x40);
        assert_eq!(
            report[common + OFFSET_AUDIO_CONTROL],
            DS_OUTPUT_AUDIO_ROUTE_SPEAKER
        );
        assert_eq!(
            report[common + OFFSET_VALID_FLAG0]
                & (DS_OUTPUT_VALID_FLAG0_SPEAKER_VOLUME_ENABLE
                    | DS_OUTPUT_VALID_FLAG0_MIC_VOLUME_ENABLE
                    | DS_OUTPUT_VALID_FLAG0_AUDIO_CONTROL_ENABLE),
            DS_OUTPUT_VALID_FLAG0_SPEAKER_VOLUME_ENABLE
                | DS_OUTPUT_VALID_FLAG0_MIC_VOLUME_ENABLE
                | DS_OUTPUT_VALID_FLAG0_AUDIO_CONTROL_ENABLE
        );
    }

    #[test]
    fn feature_reports_expose_firmware_and_pairing_details() {
        let mut firmware = [0; DS_FEATURE_REPORT_FIRMWARE_INFO_LEN];
        firmware[0] = DS_FEATURE_REPORT_FIRMWARE_INFO;
        firmware[24..28].copy_from_slice(&(0x0102_0304u32).to_le_bytes());
        firmware[28..32].copy_from_slice(&(0xa0b0_c0d0u32).to_le_bytes());
        firmware[44..46].copy_from_slice(&(0x0215u16).to_le_bytes());

        assert_eq!(
            parse_firmware_info(&firmware),
            Some(FirmwareInfo {
                hardware_version: 0x0102_0304,
                firmware_version: 0xa0b0_c0d0,
                feature_version: 0x0215,
            })
        );

        let mut pairing = [0; DS_FEATURE_REPORT_PAIRING_INFO_LEN];
        pairing[0] = DS_FEATURE_REPORT_PAIRING_INFO;
        pairing[1..7].copy_from_slice(&[6, 5, 4, 3, 2, 1]);
        assert_eq!(
            parse_pairing_mac_address(&pairing).as_deref(),
            Some("01:02:03:04:05:06")
        );
    }
}
