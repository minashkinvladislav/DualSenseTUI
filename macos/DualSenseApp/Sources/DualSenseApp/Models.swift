import Foundation

struct ServiceResponse: Decodable {
    let id: UInt64
    let ok: Bool
    let error: String?
    let snapshot: CoreSnapshot?
    let liveState: LiveState?
}

struct CoreSnapshot: Decodable {
    let appName: String
    let status: String
    let devices: [ControllerDevice]
    let selectedDevice: Int
    let profile: ControllerProfile
    let liveInput: GamepadInput?
    let inputStatus: String
    let keyboardMappingStatus: String
    let mouseMappingStatus: String
    let eventPostingGranted: Bool
    let eventPostingStatus: String
    let profilePath: String
    let savedProfiles: [SavedProfile]
    let dirty: Bool
    let audioReactive: AudioReactiveStatus
    let backgroundAgent: BackgroundAgentStatus

    var selectedController: ControllerDevice? {
        devices.indices.contains(selectedDevice) ? devices[selectedDevice] : nil
    }
}

/// The small, frequently updated portion of the GUI service response.
/// Static profile data intentionally remains in `CoreSnapshot`.
struct LiveState: Decodable {
    let liveInput: GamepadInput?
    let inputStatus: String
    let audioReactive: AudioReactiveStatus

    init(snapshot: CoreSnapshot) {
        liveInput = snapshot.liveInput
        inputStatus = snapshot.inputStatus
        audioReactive = snapshot.audioReactive
    }
}

struct ControllerDevice: Decodable, Hashable {
    let name: String
    let vendorId: UInt32
    let productId: UInt32
    let transport: String
    let macAddress: String?
    let firmware: FirmwareInfo?
    let diagnosticsError: String?

    var subtitle: String {
        let connection = transport.isEmpty ? "Unknown transport" : transport
        if let macAddress {
            return "\(connection)  \(macAddress)"
        }
        return connection
    }
}

struct FirmwareInfo: Decodable, Hashable {
    let hardwareVersion: UInt32
    let firmwareVersion: UInt32
    let featureVersion: UInt16
}

struct ControllerProfile: Decodable {
    let lightbar: RGBColor
    let haptics: HapticsProfile
    let adaptiveTriggers: AdaptiveTriggerProfile
    let system: SystemProfile
    let mappings: [ButtonMapping]
    let keyboardMapping: KeyboardMappingProfile
    let mouseMapping: MouseMappingProfile
}

struct SavedProfile: Decodable, Hashable {
    let id: String
    let name: String
}

struct RGBColor: Decodable, Equatable {
    let r: UInt8
    let g: UInt8
    let b: UInt8
}

struct HapticsProfile: Decodable {
    let enabled: Bool
    let audioHaptics: Bool
    let leftStrength: UInt8
    let rightStrength: UInt8
    let audioReactive: AudioReactiveProfile

    var strength: UInt8 {
        UInt8((UInt16(leftStrength) + UInt16(rightStrength) + 1) / 2)
    }
}

struct AudioReactiveProfile: Decodable {
    let sensitivityPercent: UInt8
    let thresholdPercent: UInt8
}

struct AdaptiveTriggerProfile: Decodable {
    let target: String
    let mode: String
    let preset: String
    let intensity: UInt8
    let startPosition: UInt8
    let endPosition: UInt8
    let frequency: UInt8
}

struct SystemProfile: Decodable {
    let playerIndicator: String
    let microphoneMuted: Bool
    let speakerVolume: UInt8
    let microphoneVolume: UInt8
    let audioRoute: String
}

struct KeyboardMappingProfile: Decodable {
    let enabled: Bool
    let bindings: [KeyboardBinding]
}

struct ButtonMapping: Decodable, Hashable {
    let from: String
    let to: String
}

struct KeyboardBinding: Decodable, Hashable {
    let from: String
    let to: String
}

struct MouseMappingProfile: Decodable {
    let enabled: Bool
    let pointerSpeed: UInt8
    let deadzonePercent: UInt8
    let scrollSpeed: UInt8
}

struct GamepadInput: Decodable, Equatable {
    let leftStick: StickInput
    let rightStick: StickInput
    let leftTrigger: UInt8
    let rightTrigger: UInt8
    let buttons: [String]
    let batteryPercent: UInt8?
    let batteryStatus: String
    let headsetConnected: Bool
    let microphoneConnected: Bool
    let microphoneMuted: Bool
    let packetCount: UInt64
}

struct StickInput: Decodable, Equatable {
    let x: UInt8
    let y: UInt8

    var normalizedX: Double {
        (Double(x) - 128.0) / 127.0
    }

    var normalizedY: Double {
        (Double(y) - 128.0) / 127.0
    }
}

struct AudioReactiveStatus: Decodable, Equatable {
    let state: String
    let running: Bool
    let low: UInt16
    let high: UInt16
}

struct BackgroundAgentStatus: Decodable {
    let installed: Bool
    let loaded: Bool
}
