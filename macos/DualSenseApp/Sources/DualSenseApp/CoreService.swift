import Combine
import Foundation

enum LivePollingMode: Equatable {
    case none
    case input
    case audio
}

@MainActor
final class AudioReactiveControlStore: ObservableObject {
    @Published private(set) var state: AudioReactiveStatus?

    func update(_ status: AudioReactiveStatus) {
        guard state?.state != status.state || state?.running != status.running else {
            return
        }
        state = status
    }
}

@MainActor
final class LiveStateStore: ObservableObject {
    @Published private(set) var state: LiveState?
    let audioControls = AudioReactiveControlStore()
    private var latestState: LiveState?
    private var mode: LivePollingMode = .input
    private var lastPacketStatusPublication = Date.distantPast
    private let packetStatusRefreshInterval: TimeInterval = 5

    func setMode(_ mode: LivePollingMode) {
        guard self.mode != mode else {
            return
        }

        self.mode = mode
        lastPacketStatusPublication = .distantPast
        if mode != .none, let latestState {
            state = latestState
        }
        if mode == .audio, let latestState {
            audioControls.update(latestState.audioReactive)
        }
    }

    func update(_ state: LiveState) {
        let previous = latestState
        latestState = state

        switch mode {
        case .none:
            return
        case .input:
            guard inputPresentationChanged(from: previous, to: state, now: Date()) else {
                return
            }
        case .audio:
            audioControls.update(state.audioReactive)
            guard audioPresentationChanged(from: previous?.audioReactive, to: state.audioReactive) else {
                return
            }
        }

        self.state = state
    }

    private func inputPresentationChanged(from previous: LiveState?, to current: LiveState, now: Date) -> Bool {
        guard let previous else {
            if isPacketStatus(current.inputStatus) {
                lastPacketStatusPublication = now
            }
            return true
        }

        if inputChanged(from: previous.liveInput, to: current.liveInput) {
            if isPacketStatus(current.inputStatus) {
                lastPacketStatusPublication = now
            }
            return true
        }

        guard previous.inputStatus != current.inputStatus else {
            return false
        }

        guard isPacketStatus(previous.inputStatus), isPacketStatus(current.inputStatus) else {
            return true
        }

        guard now.timeIntervalSince(lastPacketStatusPublication) >= packetStatusRefreshInterval else {
            return false
        }

        lastPacketStatusPublication = now
        return true
    }

    private func isPacketStatus(_ status: String) -> Bool {
        status.hasPrefix("Live input packets:")
    }

    private func inputChanged(from previous: GamepadInput?, to current: GamepadInput?) -> Bool {
        guard let previous else {
            return current != nil
        }
        guard let current else {
            return true
        }

        return stickBucket(previous.leftStick.x) != stickBucket(current.leftStick.x)
            || stickBucket(previous.leftStick.y) != stickBucket(current.leftStick.y)
            || stickBucket(previous.rightStick.x) != stickBucket(current.rightStick.x)
            || stickBucket(previous.rightStick.y) != stickBucket(current.rightStick.y)
            || previous.leftTrigger != current.leftTrigger
            || previous.rightTrigger != current.rightTrigger
            || previous.buttons != current.buttons
            || previous.batteryPercent != current.batteryPercent
            || previous.batteryStatus != current.batteryStatus
            || previous.headsetConnected != current.headsetConnected
            || previous.microphoneConnected != current.microphoneConnected
            || previous.microphoneMuted != current.microphoneMuted
    }

    private func audioPresentationChanged(
        from previous: AudioReactiveStatus?,
        to current: AudioReactiveStatus
    ) -> Bool {
        guard let previous else {
            return true
        }

        return previous.state != current.state
            || previous.running != current.running
            || audioMeterPercent(previous.low) != audioMeterPercent(current.low)
            || audioMeterPercent(previous.high) != audioMeterPercent(current.high)
    }

    // The UI renders the meters as whole percentages, so sub-percent changes
    // do not need to invalidate SwiftUI views at the polling rate.
    private func audioMeterPercent(_ value: UInt16) -> Int {
        Int((Double(value) / Double(UInt16.max) * 100).rounded())
    }

    // One bucket moves the dashboard indicator by about one screen point.
    // Smaller raw HID fluctuations are idle noise rather than useful input.
    private func stickBucket(_ value: UInt8) -> Int {
        (Int(value) - 128) / 4
    }

}

@MainActor
final class CoreService: ObservableObject {
    @Published private(set) var snapshot: CoreSnapshot?
    @Published private(set) var running = false
    @Published private(set) var launchError: String?
    @Published private(set) var backgroundServiceTransition = false
    let liveState = LiveStateStore()

    private var process: Process?
    private var input: FileHandle?
    private var output: FileHandle?
    private var errorOutput: FileHandle?
    private var outputBuffer = Data()
    private let responseDecoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()
    private var requestID: UInt64 = 1
    private var pollTimer: Timer?
    private var backgroundAgentTool: Process?
    private var livePollingMode: LivePollingMode = .input
    private let livePollingInterval: TimeInterval = 0.10

    deinit {
        pollTimer?.invalidate()
        output?.readabilityHandler = nil
        errorOutput?.readabilityHandler = nil
        process?.terminate()
    }

    func start() {
        guard !running, !backgroundServiceTransition else {
            return
        }
        guard let coreURL = coreExecutableURL() else {
            launchError = "DualSenseCore is missing from the application bundle."
            return
        }

        let process = Process()
        let inputPipe = Pipe()
        let outputPipe = Pipe()
        let errorPipe = Pipe()
        outputBuffer.removeAll(keepingCapacity: true)
        process.executableURL = coreURL
        process.arguments = ["--gui-service"]
        process.standardInput = inputPipe
        process.standardOutput = outputPipe
        process.standardError = errorPipe

        outputPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                handle.readabilityHandler = nil
                return
            }
            Task { @MainActor [weak self] in
                self?.consumeServiceOutput(data)
            }
        }
        errorPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                handle.readabilityHandler = nil
                return
            }
            let message = String(decoding: data, as: UTF8.self)
            Task { @MainActor [weak self] in
                self?.launchError = message.trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }
        process.terminationHandler = { [weak self] process in
            Task { @MainActor [weak self] in
                self?.serviceDidTerminate(process)
            }
        }

        do {
            try process.run()
            self.process = process
            input = inputPipe.fileHandleForWriting
            output = outputPipe.fileHandleForReading
            errorOutput = errorPipe.fileHandleForReading
            running = true
            launchError = nil
            if livePollingMode != .none {
                startPolling()
            }
            requestSnapshot()
        } catch {
            launchError = "Could not start DualSenseCore: \(error.localizedDescription)"
        }
    }

    func stop() {
        pollTimer?.invalidate()
        pollTimer = nil
        send("quit")
        input?.closeFile()
        output?.readabilityHandler = nil
        errorOutput?.readabilityHandler = nil
        if process?.isRunning == true {
            process?.terminate()
        }
        input = nil
        output = nil
        errorOutput = nil
        process = nil
        outputBuffer.removeAll(keepingCapacity: true)
        running = false
    }

    func requestSnapshot() {
        send("snapshot")
    }

    private func requestLiveState() {
        send("live_state")
    }

    func refresh() {
        guard !backgroundServiceTransition else {
            return
        }
        if running {
            send("refresh")
        } else {
            start()
        }
    }

    func selectDevice(_ index: Int) {
        send("select_device", fields: ["index": index])
    }

    func setLightbar(r: Int, g: Int, b: Int, apply: Bool = true) {
        send("set_lightbar", fields: ["r": r, "g": g, "b": b, "apply": apply])
    }

    func setLightbarInactive(_ inactive: Bool) {
        send("set_lightbar_inactive", fields: ["inactive": inactive])
    }

    func reapplyLightbar() {
        send("reapply_lightbar")
    }

    func setHaptics(enabled: Bool, audioHaptics: Bool, strength: Int, apply: Bool = true) {
        send(
            "set_haptics",
            fields: [
                "enabled": enabled,
                "audio_haptics": audioHaptics,
                "strength": strength,
                "apply": apply,
            ]
        )
    }

    func setAudioReactive(enabled: Bool, sensitivity: Int, threshold: Int) {
        send(
            "set_audio_reactive",
            fields: [
                "enabled": enabled,
                "sensitivity_percent": sensitivity,
                "threshold_percent": threshold,
            ]
        )
    }

    func playHapticDemo(_ demo: String) {
        send("play_haptic_demo", fields: ["demo": demo])
    }

    func setTriggers(
        target: String,
        mode: String,
        preset: String,
        intensity: Int,
        startPosition: Int,
        endPosition: Int,
        frequency: Int,
        apply: Bool = true
    ) {
        send(
            "set_triggers",
            fields: [
                "target": target,
                "mode": mode,
                "preset": preset,
                "intensity": intensity,
                "start_position": startPosition,
                "end_position": endPosition,
                "frequency": frequency,
                "apply": apply,
            ]
        )
    }

    func resetTriggers() {
        send("reset_triggers")
    }

    func setSystem(
        playerIndicator: String,
        microphoneMuted: Bool,
        speakerVolume: Int,
        microphoneVolume: Int,
        audioRoute: String,
        apply: Bool = true
    ) {
        send(
            "set_system",
            fields: [
                "player_indicator": playerIndicator,
                "microphone_muted": microphoneMuted,
                "speaker_volume": speakerVolume,
                "microphone_volume": microphoneVolume,
                "audio_route": audioRoute,
                "apply": apply,
            ]
        )
    }

    func setMouse(enabled: Bool, pointerSpeed: Int, deadzone: Int, scrollSpeed: Int) {
        send(
            "set_mouse",
            fields: [
                "enabled": enabled,
                "pointer_speed": pointerSpeed,
                "deadzone_percent": deadzone,
                "scroll_speed": scrollSpeed,
            ]
        )
    }

    func setControllerMapping(from: String, to: String) {
        send("set_controller_mapping", fields: ["from": from, "to": to])
    }

    func setKeyboardMapping(from: String, to: String) {
        send("set_keyboard_mapping", fields: ["from": from, "to": to])
    }

    func setKeyboardOutput(enabled: Bool) {
        send("set_keyboard_output", fields: ["enabled": enabled])
    }

    func requestEventPostingAccess() {
        send("request_event_posting_access")
    }

    func openAccessibilitySettings() {
        send("open_accessibility_settings")
    }

    func saveProfile() {
        send("save_profile")
    }

    func saveNamedProfile(name: String) {
        send("save_named_profile", fields: ["name": name])
    }

    func loadSavedProfile(id: String) {
        send("load_named_profile", fields: ["profile_id": id])
    }

    func resetProfile() {
        send("reset_profile")
    }

    func installBackgroundAgent() {
        changeBackgroundService(argument: "--install-agent", action: "enable")
    }

    func uninstallBackgroundAgent() {
        changeBackgroundService(argument: "--uninstall-agent", action: "disable")
    }

    func refreshBackgroundStatus() {
        guard !backgroundServiceTransition else {
            return
        }
        if running {
            send("background_status")
        } else {
            start()
        }
    }

    /// Poll only while a screen needs live state. Dashboard and Haptics publish
    /// different fields so background packet counters cannot redraw either view.
    func setLivePollingMode(_ mode: LivePollingMode) {
        guard livePollingMode != mode else {
            return
        }

        livePollingMode = mode
        liveState.setMode(mode)
        guard running else {
            return
        }

        if mode != .none {
            startPolling()
            requestSnapshot()
        } else {
            stopPolling()
        }
    }

    private func startPolling() {
        stopPolling()
        let timer = Timer.scheduledTimer(withTimeInterval: livePollingInterval, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self, self.livePollingMode != .none else {
                    return
                }
                self.requestLiveState()
            }
        }
        timer.tolerance = livePollingInterval * 0.2
        pollTimer = timer
    }

    private func stopPolling() {
        pollTimer?.invalidate()
        pollTimer = nil
    }

    private func changeBackgroundService(argument: String, action: String) {
        guard !backgroundServiceTransition else {
            return
        }
        guard coreExecutableURL() != nil else {
            launchError = "DualSenseCore is missing from the application bundle."
            return
        }

        // Stop the GUI transport first. The external helper can then safely
        // bootstrap or boot out the daemon without killing an active proxy.
        backgroundServiceTransition = true
        launchError = nil
        let coreProcess = process
        stop()

        guard let coreProcess else {
            runBackgroundServiceTool(argument: argument, action: action)
            return
        }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            coreProcess.waitUntilExit()
            Task { @MainActor [weak self] in
                self?.runBackgroundServiceTool(argument: argument, action: action)
            }
        }
    }

    private func runBackgroundServiceTool(argument: String, action: String) {
        guard let coreURL = coreExecutableURL() else {
            backgroundServiceTransition = false
            launchError = "DualSenseCore is missing from the application bundle."
            start()
            return
        }

        let process = Process()
        let outputPipe = Pipe()
        let errorPipe = Pipe()
        process.executableURL = coreURL
        process.arguments = [argument]
        process.standardOutput = outputPipe
        process.standardError = errorPipe
        process.terminationHandler = { [weak self] completedProcess in
            let output = outputPipe.fileHandleForReading.readDataToEndOfFile()
            let errorOutput = errorPipe.fileHandleForReading.readDataToEndOfFile()
            let detail = [output, errorOutput]
                .map { String(decoding: $0, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
                .joined(separator: " ")

            Task { @MainActor [weak self] in
                self?.finishBackgroundServiceTool(
                    completedProcess,
                    action: action,
                    detail: detail
                )
            }
        }

        backgroundAgentTool = process
        do {
            try process.run()
        } catch {
            backgroundAgentTool = nil
            backgroundServiceTransition = false
            start()
            launchError = "Could not \(action) Background Service: \(error.localizedDescription)"
        }
    }

    private func finishBackgroundServiceTool(_ process: Process, action: String, detail: String) {
        guard backgroundAgentTool === process else {
            return
        }

        backgroundAgentTool = nil
        backgroundServiceTransition = false
        start()

        guard process.terminationStatus == 0 else {
            let suffix = detail.isEmpty ? "" : " \(detail)"
            launchError = "Could not \(action) Background Service.\(suffix)"
            return
        }

        launchError = nil
    }

    private func coreExecutableURL() -> URL? {
        if let override = ProcessInfo.processInfo.environment["DUALSENSE_CORE_PATH"], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }

        let bundledCore = Bundle.main.bundleURL
            .appendingPathComponent("Contents", isDirectory: true)
            .appendingPathComponent("MacOS", isDirectory: true)
            .appendingPathComponent("DualSenseCore", isDirectory: false)
        return FileManager.default.isExecutableFile(atPath: bundledCore.path) ? bundledCore : nil
    }

    private func send(_ command: String, fields: [String: Any] = [:]) {
        guard running, let input else {
            return
        }

        var payload = fields
        payload["id"] = requestID
        payload["command"] = command
        requestID &+= 1

        do {
            let data = try JSONSerialization.data(withJSONObject: payload, options: [])
            input.write(data)
            input.write(Data([0x0a]))
        } catch {
            launchError = "Could not encode command: \(error.localizedDescription)"
        }
    }

    private func consumeServiceOutput(_ data: Data) {
        outputBuffer.append(data)

        while let newline = outputBuffer.firstIndex(of: 0x0a) {
            let line = outputBuffer.prefix(upTo: newline)
            outputBuffer.removeSubrange(...newline)
            guard !line.isEmpty else {
                continue
            }
            decodeResponse(Data(line))
        }
    }

    private func decodeResponse(_ data: Data) {
        do {
            let response = try responseDecoder.decode(ServiceResponse.self, from: data)
            if let snapshot = response.snapshot {
                self.snapshot = snapshot
                liveState.update(LiveState(snapshot: snapshot))
            }
            if let state = response.liveState {
                liveState.update(state)
            }
            if response.ok {
                if launchError != nil {
                    launchError = nil
                }
            } else {
                let message = response.error ?? response.snapshot?.status ?? "DualSenseCore rejected the request."
                if launchError != message {
                    launchError = message
                }
            }
        } catch {
            launchError = "Could not decode service response: \(error.localizedDescription)"
        }
    }

    private func serviceDidTerminate(_ process: Process) {
        guard self.process === process else {
            return
        }
        stopPolling()
        output?.readabilityHandler = nil
        errorOutput?.readabilityHandler = nil
        running = false
        if process.terminationStatus != 0 {
            launchError = "DualSenseCore stopped with exit code \(process.terminationStatus)."
        }
    }
}
