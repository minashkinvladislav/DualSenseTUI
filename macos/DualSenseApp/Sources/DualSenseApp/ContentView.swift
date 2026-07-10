import AppKit
import SwiftUI

private enum AppSection: String, CaseIterable, Identifiable {
    case dashboard
    case lightbar
    case haptics
    case triggers
    case mouse
    case mapping
    case system
    case profiles

    var id: Self { self }

    var title: String {
        switch self {
        case .dashboard: return "Dashboard"
        case .lightbar: return "Lightbar"
        case .haptics: return "Haptics"
        case .triggers: return "Adaptive Triggers"
        case .mouse: return "Mouse Control"
        case .mapping: return "Mappings"
        case .system: return "System"
        case .profiles: return "Profiles"
        }
    }

    var symbol: String {
        switch self {
        case .dashboard: return "gamecontroller.fill"
        case .lightbar: return "lightbulb.2.fill"
        case .haptics: return "waveform.path"
        case .triggers: return "scope"
        case .mouse: return "cursorarrow.motionlines"
        case .mapping: return "keyboard"
        case .system: return "slider.horizontal.3"
        case .profiles: return "externaldrive.fill"
        }
    }

    var livePollingMode: LivePollingMode {
        switch self {
        case .dashboard:
            return .input
        case .haptics:
            return .audio
        default:
            return .none
        }
    }
}

struct ContentView: View {
    @EnvironmentObject private var service: CoreService
    @AppStorage("retainLightbarWhileInactive") private var retainLightbarWhileInactive = true
    @State private var selection: AppSection? = .dashboard
    @State private var lastLightbarFocusRefresh = Date.distantPast

    var body: some View {
        NavigationSplitView {
            List(AppSection.allCases, selection: $selection) { section in
                Label(section.title, systemImage: section.symbol)
                    .tag(section)
            }
            .navigationTitle("DualSenseTUI")
            .safeAreaInset(edge: .bottom) {
                SidebarStatus(snapshot: service.snapshot, running: service.running)
            }
        } detail: {
            detail
                .toolbar {
                    ToolbarItemGroup(placement: .primaryAction) {
                        Button(action: service.refresh) {
                            Image(systemName: "arrow.clockwise")
                        }
                        .help("Refresh controllers")

                        Button(action: service.saveProfile) {
                            Image(systemName: "square.and.arrow.down")
                        }
                        .help("Save controller profile")
                        .disabled(service.snapshot?.dirty != true)
                    }
                }
        }
        .background(SidebarToggleRemoval())
        .onAppear {
            service.setLivePollingMode(selection?.livePollingMode ?? .none)
            syncLightbarKeepalive()
        }
        .onChange(of: selection) { selectedSection in
            service.setLivePollingMode(selectedSection?.livePollingMode ?? .none)
        }
        .onChange(of: retainLightbarWhileInactive) { _ in
            syncLightbarKeepalive()
        }
        .onChange(of: service.snapshot?.backgroundAgent.loaded) { _ in
            syncLightbarKeepalive()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didResignActiveNotification)) { _ in
            refreshLightbarAfterFocusChange()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            updateInactiveLightbarState(false)
        }
        .onReceive(
            NSWorkspace.shared.notificationCenter.publisher(
                for: NSWorkspace.didActivateApplicationNotification
            )
        ) { _ in
            // NSApplication only reports a transition out of this app. Workspace
            // notifications also cover focus changes while DualSenseTUI is
            // already inactive, avoiding the daemon's periodic fallback delay.
            guard !NSApp.isActive else {
                return
            }
            refreshLightbarAfterFocusChange()
        }
    }

    private func updateInactiveLightbarState(_ inactive: Bool) {
        guard let snapshot = service.snapshot, !snapshot.backgroundAgent.loaded else {
            return
        }
        service.setLightbarInactive(retainLightbarWhileInactive && inactive)
    }

    private func syncLightbarKeepalive() {
        guard let snapshot = service.snapshot else {
            return
        }

        if snapshot.backgroundAgent.loaded {
            // The daemon, not the foreground window, owns the periodic LED
            // refresh. Focus changes must not start a new lightbar fade.
            service.setLightbarInactive(retainLightbarWhileInactive)
        } else {
            updateInactiveLightbarState(!NSApp.isActive)
        }
    }

    private func refreshLightbarAfterFocusChange() {
        guard retainLightbarWhileInactive else {
            return
        }

        let now = Date()
        guard now.timeIntervalSince(lastLightbarFocusRefresh) >= 0.15 else {
            return
        }
        lastLightbarFocusRefresh = now

        if service.snapshot?.backgroundAgent.loaded == true {
            service.reapplyLightbar()
        } else {
            updateInactiveLightbarState(true)
        }
    }

    @ViewBuilder
    private var detail: some View {
        if let error = service.launchError, !service.running {
            ServiceErrorView(message: error, retry: service.start)
        } else if let snapshot = service.snapshot {
            switch selection ?? .dashboard {
            case .dashboard:
                DashboardView(snapshot: snapshot, liveState: service.liveState)
            case .lightbar:
                LightbarView(snapshot: snapshot)
            case .haptics:
                HapticsView(snapshot: snapshot, liveState: service.liveState)
            case .triggers:
                TriggersView(snapshot: snapshot)
            case .mouse:
                MouseControlView(snapshot: snapshot)
            case .mapping:
                MappingView(snapshot: snapshot)
            case .system:
                SystemControlsView(snapshot: snapshot)
            case .profiles:
                ProfilesView(snapshot: snapshot)
            }
        } else {
            ProgressView("Connecting to DualSenseCore")
                .controlSize(.large)
        }
    }
}

private struct SidebarToggleRemoval: NSViewRepresentable {
    func makeNSView(context: Context) -> SidebarToggleRemovalView {
        SidebarToggleRemovalView(frame: .zero)
    }

    func updateNSView(_ nsView: SidebarToggleRemovalView, context: Context) {
        nsView.removeSidebarToggle()
    }
}

private final class SidebarToggleRemovalView: NSView {
    private weak var observedToolbar: NSToolbar?
    private var hasRemovedSidebarToggle = false

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        DispatchQueue.main.async { [weak self] in
            self?.removeSidebarToggle()
        }
    }

    func removeSidebarToggle() {
        guard let toolbar = window?.toolbar else {
            return
        }
        if observedToolbar !== toolbar {
            observedToolbar = toolbar
            hasRemovedSidebarToggle = false
        }
        guard !hasRemovedSidebarToggle,
              let index = toolbar.items.firstIndex(where: {
                  $0.itemIdentifier == .toggleSidebar
              }) else {
            return
        }
        toolbar.removeItem(at: index)
        hasRemovedSidebarToggle = true
    }
}

private struct SidebarStatus: View {
    let snapshot: CoreSnapshot?
    let running: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(
                snapshot?.selectedController?.name ?? "No controller",
                systemImage: snapshot?.selectedController == nil ? "gamecontroller" : "gamecontroller.fill"
            )
            .font(.caption)
            .lineLimit(1)

            HStack(spacing: 5) {
                Circle()
                    .fill(snapshot?.selectedController == nil ? Color.secondary : Color.green)
                    .frame(width: 7, height: 7)
                Text(running ? "Core running" : "Core stopped")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct ServiceErrorView: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 42))
                .foregroundStyle(.orange)
            Text("DualSenseCore is unavailable")
                .font(.title2.weight(.semibold))
            Text(message)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .frame(maxWidth: 480)
            Button("Try Again", action: retry)
        }
        .padding(32)
    }
}

private struct DashboardView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot
    let liveState: LiveStateStore

    var body: some View {
        Form {
            Section("Controller") {
                if snapshot.devices.isEmpty {
                    ContentUnavailableControllerView(refresh: service.refresh)
                } else {
                    Picker(
                        "Active controller",
                        selection: Binding(
                            get: { snapshot.selectedDevice },
                            set: { service.selectDevice($0) }
                        )
                    ) {
                        ForEach(Array(snapshot.devices.enumerated()), id: \.offset) { index, device in
                            Text(device.name).tag(index)
                        }
                    }

                    if let device = snapshot.selectedController {
                        LabeledContent("Connection", value: device.subtitle)
                        LabeledContent("Product", value: String(format: "%04x:%04x", device.vendorId, device.productId))
                        if let error = device.diagnosticsError {
                            LabeledContent("Diagnostics", value: error)
                        }
                    }
                }
            }

            Section("Live Input") {
                LiveInputSection(snapshot: snapshot, liveState: liveState)
            }

            Section("Profile") {
                LabeledContent("Status", value: snapshot.dirty ? "Unsaved changes" : "Saved")
                LabeledContent("Location", value: snapshot.profilePath)
                Button("Save Profile", action: service.saveProfile)
                    .disabled(!snapshot.dirty)
            }

            Section("Activity") {
                Text(snapshot.status)
                    .textSelection(.enabled)
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Dashboard")
    }
}

private struct LiveInputSection: View {
    let snapshot: CoreSnapshot
    @ObservedObject var liveState: LiveStateStore

    var body: some View {
        let currentLiveState = liveState.state ?? LiveState(snapshot: snapshot)

        if let input = currentLiveState.liveInput {
            ControllerInputView(input: input)
            LabeledContent("Input status", value: currentLiveState.inputStatus)
        } else {
            Text(currentLiveState.inputStatus)
                .foregroundStyle(.secondary)
        }
    }
}

private struct ContentUnavailableControllerView: View {
    let refresh: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "gamecontroller")
                .font(.title2)
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text("No DualSense detected")
                Text("Connect a controller over USB or Bluetooth.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Button("Refresh", action: refresh)
        }
    }
}

private struct LightbarView: View {
    @EnvironmentObject private var service: CoreService
    @AppStorage("retainLightbarWhileInactive") private var retainLightbarWhileInactive = true
    let snapshot: CoreSnapshot

    private var profile: ControllerProfile { snapshot.profile }

    var body: some View {
        Form {
            Section("Color") {
                LightbarSwatch(color: profile.lightbar)
                ColorSlider(
                    label: "Red",
                    value: Int(profile.lightbar.r),
                    tint: .red
                ) { value in
                    service.setLightbar(r: value, g: Int(profile.lightbar.g), b: Int(profile.lightbar.b))
                }
                ColorSlider(
                    label: "Green",
                    value: Int(profile.lightbar.g),
                    tint: .green
                ) { value in
                    service.setLightbar(r: Int(profile.lightbar.r), g: value, b: Int(profile.lightbar.b))
                }
                ColorSlider(
                    label: "Blue",
                    value: Int(profile.lightbar.b),
                    tint: .blue
                ) { value in
                    service.setLightbar(r: Int(profile.lightbar.r), g: Int(profile.lightbar.g), b: value)
                }
            }

            Section {
                Toggle("Keep color when app is inactive", isOn: $retainLightbarWhileInactive)
                Button("Apply Lightbar") {
                    service.setLightbar(
                        r: Int(profile.lightbar.r),
                        g: Int(profile.lightbar.g),
                        b: Int(profile.lightbar.b)
                    )
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Lightbar")
        .disabled(snapshot.devices.isEmpty)
    }
}

private struct HapticsView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot
    let liveState: LiveStateStore

    private var profile: HapticsProfile { snapshot.profile.haptics }

    var body: some View {
        Form {
            Section("Output") {
                Toggle(
                    "Enable haptics",
                    isOn: Binding(
                        get: { profile.enabled },
                        set: { enabled in
                            service.setHaptics(
                                enabled: enabled,
                                audioHaptics: profile.audioHaptics,
                                strength: Int(profile.strength)
                            )
                        }
                    )
                )
                Picker(
                    "Protocol",
                    selection: Binding(
                        get: { profile.audioHaptics },
                        set: { audioHaptics in
                            service.setHaptics(
                                enabled: profile.enabled,
                                audioHaptics: audioHaptics,
                                strength: Int(profile.strength)
                            )
                        }
                    )
                ) {
                    Text("Haptic v2").tag(true)
                    Text("Legacy rumble").tag(false)
                }
                .pickerStyle(.segmented)
                ValueSlider(
                    label: "Strength",
                    value: Int(profile.strength),
                    range: 0 ... 255
                ) { strength in
                    service.setHaptics(
                        enabled: profile.enabled,
                        audioHaptics: profile.audioHaptics,
                        strength: strength
                    )
                }
            }

            Section("Demos") {
                VStack(alignment: .leading, spacing: 16) {
                    HapticDemoGroup(title: "Taps & impacts", demos: HapticDemoOption.impactDemos) { demo in
                        service.playHapticDemo(demo.rawValue)
                    }
                    Divider()
                    HapticDemoGroup(title: "Patterns", demos: HapticDemoOption.patternDemos) { demo in
                        service.playHapticDemo(demo.rawValue)
                    }
                }
                .frame(maxWidth: 620, alignment: .leading)
                .padding(.vertical, 2)
            }

            Section("System Audio") {
                AudioReactiveHapticsSection(
                    profile: profile,
                    fallbackAudioReactive: snapshot.audioReactive,
                    liveState: liveState,
                    audioControls: liveState.audioControls
                )
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Haptics")
        .disabled(snapshot.devices.isEmpty)
    }
}

private struct AudioReactiveHapticsSection: View {
    @EnvironmentObject private var service: CoreService
    let profile: HapticsProfile
    let fallbackAudioReactive: AudioReactiveStatus
    let liveState: LiveStateStore
    @ObservedObject var audioControls: AudioReactiveControlStore

    private var audioReactive: AudioReactiveStatus {
        audioControls.state ?? fallbackAudioReactive
    }

    var body: some View {
        Toggle(
            "Audio-reactive haptics",
            isOn: Binding(
                get: { audioReactive.running },
                set: { enabled in
                    service.setAudioReactive(
                        enabled: enabled,
                        sensitivity: Int(profile.audioReactive.sensitivityPercent),
                        threshold: Int(profile.audioReactive.thresholdPercent)
                    )
                }
            )
        )
        LabeledContent("State", value: audioReactive.state)
        ValueSlider(
            label: "Sensitivity",
            value: Int(profile.audioReactive.sensitivityPercent),
            range: 25 ... 250,
            suffix: "%"
        ) { value in
            service.setAudioReactive(
                enabled: audioReactive.running,
                sensitivity: value,
                threshold: Int(profile.audioReactive.thresholdPercent)
            )
        }
        ValueSlider(
            label: "Noise gate",
            value: Int(profile.audioReactive.thresholdPercent),
            range: 0 ... 90,
            suffix: "%"
        ) { value in
            service.setAudioReactive(
                enabled: audioReactive.running,
                sensitivity: Int(profile.audioReactive.sensitivityPercent),
                threshold: value
            )
        }
        AudioReactiveMeter(
            fallbackAudioReactive: fallbackAudioReactive,
            liveState: liveState
        )
    }
}

/// Keeps live meter invalidation scoped to the two progress rows rather than
/// rebuilding the surrounding toggles and sliders ten times per second.
private struct AudioReactiveMeter: View {
    let fallbackAudioReactive: AudioReactiveStatus
    @ObservedObject var liveState: LiveStateStore

    var body: some View {
        let audioReactive = liveState.state?.audioReactive ?? fallbackAudioReactive
        AudioMeter(low: audioReactive.low, high: audioReactive.high)
    }
}

private struct TriggersView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot

    private var profile: AdaptiveTriggerProfile { snapshot.profile.adaptiveTriggers }

    var body: some View {
        Form {
            Section("Effect") {
                StringPicker(
                    label: "Target",
                    selection: profile.target,
                    options: [("Both", "Both"), ("Left trigger", "Left"), ("Right trigger", "Right")]
                ) { target in
                    apply(target: target)
                }
                StringPicker(
                    label: "Mode",
                    selection: profile.mode,
                    options: [("Presets", "Preset"), ("Resistance", "Resistance"), ("Vibration", "Vibration")]
                ) { mode in
                    apply(mode: mode)
                }
                if profile.mode == "Preset" {
                    StringPicker(
                        label: "Preset",
                        selection: profile.preset,
                        options: TriggerPresetOption.allCases.map { ($0.title, $0.rawValue) }
                    ) { preset in
                        apply(preset: preset)
                    }
                }
                ValueSlider(label: "Intensity", value: Int(profile.intensity), range: 1 ... 255) { intensity in
                    apply(intensity: intensity)
                }
            }

            if profile.mode != "Preset" {
                Section("Custom Parameters") {
                    ValueSlider(label: "Start", value: Int(profile.startPosition), range: 0 ... 9) { start in
                        apply(startPosition: start, endPosition: max(start, Int(profile.endPosition)))
                    }
                    ValueSlider(label: "End", value: Int(profile.endPosition), range: Int(profile.startPosition) ... 9) { end in
                        apply(endPosition: end)
                    }
                    if profile.mode == "Vibration" {
                        ValueSlider(label: "Frequency", value: Int(profile.frequency), range: 1 ... 255) { frequency in
                            apply(frequency: frequency)
                        }
                    }
                }
            }

            Section {
                HStack {
                    Button("Apply Effect") {
                        apply()
                    }
                    Button("Reset") {
                        service.resetTriggers()
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Adaptive Triggers")
        .disabled(snapshot.devices.isEmpty)
    }

    private func apply(
        target: String? = nil,
        mode: String? = nil,
        preset: String? = nil,
        intensity: Int? = nil,
        startPosition: Int? = nil,
        endPosition: Int? = nil,
        frequency: Int? = nil
    ) {
        service.setTriggers(
            target: target ?? profile.target,
            mode: mode ?? profile.mode,
            preset: preset ?? profile.preset,
            intensity: intensity ?? Int(profile.intensity),
            startPosition: startPosition ?? Int(profile.startPosition),
            endPosition: endPosition ?? Int(profile.endPosition),
            frequency: frequency ?? Int(profile.frequency)
        )
    }
}

private struct MouseControlView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot

    private var mouse: MouseMappingProfile { snapshot.profile.mouseMapping }

    var body: some View {
        Form {
            Section("Output") {
                Toggle(
                    "Enable mouse control",
                    isOn: Binding(
                        get: { mouse.enabled },
                        set: { enabled in
                            service.setMouse(
                                enabled: enabled,
                                pointerSpeed: Int(mouse.pointerSpeed),
                                deadzone: Int(mouse.deadzonePercent),
                                scrollSpeed: Int(mouse.scrollSpeed)
                            )
                        }
                    )
                )
                LabeledContent("Permission", value: snapshot.eventPostingStatus)
                HStack {
                    Button("Grant Accessibility") {
                        service.requestEventPostingAccess()
                    }
                    Button("Open Settings") {
                        service.openAccessibilitySettings()
                    }
                }
                .disabled(snapshot.eventPostingGranted)
            }

            Section("Sensitivity") {
                ValueSlider(label: "Pointer speed", value: Int(mouse.pointerSpeed), range: 1 ... 40) { value in
                    service.setMouse(
                        enabled: mouse.enabled,
                        pointerSpeed: value,
                        deadzone: Int(mouse.deadzonePercent),
                        scrollSpeed: Int(mouse.scrollSpeed)
                    )
                }
                ValueSlider(label: "Dead zone", value: Int(mouse.deadzonePercent), range: 0 ... 40, suffix: "%") { value in
                    service.setMouse(
                        enabled: mouse.enabled,
                        pointerSpeed: Int(mouse.pointerSpeed),
                        deadzone: value,
                        scrollSpeed: Int(mouse.scrollSpeed)
                    )
                }
                ValueSlider(label: "Scroll speed", value: Int(mouse.scrollSpeed), range: 1 ... 20) { value in
                    service.setMouse(
                        enabled: mouse.enabled,
                        pointerSpeed: Int(mouse.pointerSpeed),
                        deadzone: Int(mouse.deadzonePercent),
                        scrollSpeed: value
                    )
                }
            }

            Section("Controls") {
                LabeledContent("Pointer", value: "Left stick")
                LabeledContent("Scroll", value: "Right stick vertical")
                LabeledContent("Primary click", value: "Cross")
                LabeledContent("Secondary click", value: "Circle")
                LabeledContent("Middle click", value: "Square")
                LabeledContent("Drag", value: "Hold a click and move left stick")
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Mouse Control")
    }
}

private enum MappingRowMetrics {
    static let height: CGFloat = 28
    static let horizontalInset: CGFloat = 16
}

private struct MappingView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot

    private var profile: ControllerProfile { snapshot.profile }

    var body: some View {
        Form {
            Section("Keyboard Output") {
                Toggle(
                    "Enable keyboard output",
                    isOn: Binding(
                        get: { profile.keyboardMapping.enabled },
                        set: { service.setKeyboardOutput(enabled: $0) }
                    )
                )
                LabeledContent("Status", value: snapshot.keyboardMappingStatus)
            }

            Section("Accessibility") {
                LabeledContent("Permission", value: snapshot.eventPostingStatus)
                HStack {
                    Button("Grant Accessibility") {
                        service.requestEventPostingAccess()
                    }
                    Button("Open Settings") {
                        service.openAccessibilitySettings()
                    }
                }
                .disabled(snapshot.eventPostingGranted)
            }

            Section("Keyboard Bindings") {
                ForEach(profile.keyboardMapping.bindings, id: \.from) { binding in
                    MappingPicker(
                        source: buttonTitle(binding.from),
                        selection: binding.to,
                        options: keyboardKeyOptions
                    ) { target in
                        service.setKeyboardMapping(from: binding.from, to: target)
                    }
                }
            }

            Section("Controller Profile") {
                ForEach(profile.mappings, id: \.from) { mapping in
                    MappingPicker(
                        source: buttonTitle(mapping.from),
                        selection: mapping.to,
                        options: buttonOptions
                    ) { target in
                        service.setControllerMapping(from: mapping.from, to: target)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .environment(\.defaultMinListRowHeight, MappingRowMetrics.height)
        .transaction { $0.animation = nil }
        .navigationTitle("Mappings")
    }
}

private struct SystemControlsView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot

    private var profile: SystemProfile { snapshot.profile.system }

    var body: some View {
        Form {
            Section("Controller") {
                StringPicker(
                    label: "Player LEDs",
                    selection: profile.playerIndicator,
                    options: [
                        ("Off", "Off"),
                        ("Player 1", "Player1"),
                        ("Player 2", "Player2"),
                        ("Player 3", "Player3"),
                        ("Player 4", "Player4"),
                        ("Player 5", "Player5"),
                    ]
                ) { indicator in
                    apply(playerIndicator: indicator)
                }
                Toggle(
                    "Mute microphone",
                    isOn: Binding(
                        get: { profile.microphoneMuted },
                        set: { muted in apply(microphoneMuted: muted) }
                    )
                )
            }

            Section("Levels") {
                ValueSlider(label: "Controller speaker", value: Int(profile.speakerVolume), range: 0 ... 255) { value in
                    apply(speakerVolume: value)
                }
                ValueSlider(label: "Microphone", value: Int(profile.microphoneVolume), range: 0 ... 64) { value in
                    apply(microphoneVolume: value)
                }
            }

            Section("Audio Route") {
                StringPicker(
                    label: "Output",
                    selection: profile.audioRoute,
                    options: [
                        ("Keep current", "Unchanged"),
                        ("Headphones", "Headphones"),
                        ("Controller speaker", "Speaker"),
                    ]
                ) { route in
                    apply(audioRoute: route)
                }
            }

            Section {
                Button("Apply System Controls") {
                    apply()
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("System")
        .disabled(snapshot.devices.isEmpty)
    }

    private func apply(
        playerIndicator: String? = nil,
        microphoneMuted: Bool? = nil,
        speakerVolume: Int? = nil,
        microphoneVolume: Int? = nil,
        audioRoute: String? = nil
    ) {
        service.setSystem(
            playerIndicator: playerIndicator ?? profile.playerIndicator,
            microphoneMuted: microphoneMuted ?? profile.microphoneMuted,
            speakerVolume: speakerVolume ?? Int(profile.speakerVolume),
            microphoneVolume: microphoneVolume ?? Int(profile.microphoneVolume),
            audioRoute: audioRoute ?? profile.audioRoute
        )
    }
}

private struct ProfilesView: View {
    @EnvironmentObject private var service: CoreService
    let snapshot: CoreSnapshot
    @State private var selectedProfileID: String?
    @State private var newProfileName = ""
    @State private var pendingProfileID: String?
    @State private var isLoadConfirmationPresented = false
    @State private var isResetConfirmationPresented = false

    private var selectedProfile: SavedProfile? {
        if let selectedProfileID,
           let selectedProfile = snapshot.savedProfiles.first(where: { $0.id == selectedProfileID }) {
            return selectedProfile
        }
        return snapshot.savedProfiles.first
    }

    private var savedProfileSelection: Binding<String> {
        Binding(
            get: { selectedProfile?.id ?? "" },
            set: { selectedProfileID = $0 }
        )
    }

    var body: some View {
        Form {
            Section("Controller Profile") {
                LabeledContent("Controller file", value: snapshot.profilePath)
                LabeledContent(
                    "Auto-apply",
                    value: snapshot.dirty ? "Unsaved changes" : "Saved for this controller"
                )
                HStack {
                    Button("Save for Controller", action: service.saveProfile)
                        .disabled(!snapshot.dirty)
                    Button("Restore Defaults", role: .destructive) {
                        isResetConfirmationPresented = true
                    }
                }
            }

            Section("Profile Library") {
                Picker("Library profile", selection: savedProfileSelection) {
                    if snapshot.savedProfiles.isEmpty {
                        Text("No reusable profiles").tag("")
                    } else {
                        ForEach(snapshot.savedProfiles, id: \.id) { profile in
                            Text(profile.name).tag(profile.id)
                        }
                    }
                }
                .disabled(snapshot.savedProfiles.isEmpty)

                Button("Load Library Profile", action: requestSelectedProfileLoad)
                    .disabled(selectedProfile == nil)

                TextField("Profile name", text: $newProfileName)
                Button("Save to Library") {
                    service.saveNamedProfile(name: newProfileName)
                }
                .disabled(newProfileName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }

            Section("Activity") {
                Text(snapshot.status)
                    .textSelection(.enabled)
            }

            Section("Background Service") {
                Toggle(
                    "Keep mappings active in background",
                    isOn: Binding(
                        get: { snapshot.backgroundAgent.loaded },
                        set: { enabled in
                            if enabled {
                                service.installBackgroundAgent()
                            } else {
                                service.uninstallBackgroundAgent()
                            }
                        }
                    )
                )
                .disabled(service.backgroundServiceTransition)
                LabeledContent(
                    "Status",
                    value: service.backgroundServiceTransition ? "Updating..." : backgroundStatus
                )
                Button("Refresh Status", action: service.refreshBackgroundStatus)
                    .disabled(service.backgroundServiceTransition)
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Profiles")
        .onAppear(perform: selectAvailableProfile)
        .onChange(of: snapshot.savedProfiles.map(\.id)) { _ in
            selectAvailableProfile()
        }
        .confirmationDialog(
            "Discard unsaved changes?",
            isPresented: $isLoadConfirmationPresented,
            titleVisibility: .visible
        ) {
            Button("Load Library Profile", role: .destructive) {
                guard let pendingProfileID else {
                    return
                }
                service.loadSavedProfile(id: pendingProfileID)
                self.pendingProfileID = nil
            }
        } message: {
            Text("Current edits will be replaced by the saved profile.")
        }
        .confirmationDialog(
            "Restore default settings?",
            isPresented: $isResetConfirmationPresented,
            titleVisibility: .visible
        ) {
            Button("Restore Defaults", role: .destructive) {
                service.resetProfile()
            }
        } message: {
            Text("Current profile settings will be replaced. Save Profile persists the defaults.")
        }
    }

    private func requestSelectedProfileLoad() {
        guard let profile = selectedProfile else {
            return
        }

        if snapshot.dirty {
            pendingProfileID = profile.id
            isLoadConfirmationPresented = true
        } else {
            service.loadSavedProfile(id: profile.id)
        }
    }

    private func selectAvailableProfile() {
        selectedProfileID = selectedProfile?.id
    }

    private var backgroundStatus: String {
        if snapshot.backgroundAgent.loaded {
            return "Active"
        }
        if snapshot.backgroundAgent.installed {
            return "Installed"
        }
        return "Disabled"
    }
}

private struct ControllerInputView: View {
    let input: GamepadInput

    var body: some View {
        ViewThatFits(in: .horizontal) {
            fullLayout
            compactLayout
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 6)
    }

    private var fullLayout: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 28) {
                StickMonitor(title: "Left stick", stick: input.leftStick)
                DPadMonitor(buttons: pressedButtons)
                FaceButtonCluster(buttons: pressedButtons)
                StickMonitor(title: "Right stick", stick: input.rightStick)
                TriggerAndBatteryView(input: input)
            }
            pressedButtonsLabel
        }
    }

    private var compactLayout: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 16) {
                StickMonitor(title: "Left stick", stick: input.leftStick)
                DPadMonitor(buttons: pressedButtons)
                FaceButtonCluster(buttons: pressedButtons)
                StickMonitor(title: "Right stick", stick: input.rightStick)
            }
            HStack(alignment: .top, spacing: 18) {
                TriggerAndBatteryView(input: input)
                pressedButtonsLabel
            }
        }
    }

    private var pressedButtons: Set<String> {
        Set(input.buttons)
    }

    private var pressedButtonsLabel: some View {
        Text(input.buttons.isEmpty ? "No buttons pressed" : input.buttons.joined(separator: ", "))
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(2)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct StickMonitor: View {
    let title: String
    let stick: StickInput

    var body: some View {
        VStack(spacing: 6) {
            GeometryReader { proxy in
                let side = min(proxy.size.width, proxy.size.height)
                let radius = side * 0.36
                Circle()
                    .fill(Color.secondary.opacity(0.1))
                    .overlay(Circle().stroke(Color.secondary.opacity(0.3), lineWidth: 1))
                    .overlay(alignment: .center) {
                        Circle()
                            .fill(Color.accentColor)
                            .frame(width: side * 0.18, height: side * 0.18)
                            .offset(
                                x: CGFloat(stick.normalizedX) * radius,
                                y: CGFloat(stick.normalizedY) * radius
                            )
                    }
            }
            .frame(width: 108, height: 108)
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

private struct ControllerButton: View {
    let title: String
    let active: Bool
    let size: CGFloat

    init(title: String, active: Bool, size: CGFloat = 42) {
        self.title = title
        self.active = active
        self.size = size
    }

    var body: some View {
        Circle()
            .fill(active ? Color.accentColor : Color.secondary.opacity(0.12))
            .overlay {
                Text(title)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(active ? .white : .primary)
            }
            .frame(width: size, height: size)
    }
}

private struct FaceButtonCluster: View {
    let buttons: Set<String>

    var body: some View {
        VStack(spacing: 6) {
            VStack(spacing: 4) {
                ControllerButton(title: "Tri", active: buttons.contains("Triangle"), size: 32)
                HStack(spacing: 4) {
                    ControllerButton(title: "Sq", active: buttons.contains("Square"), size: 32)
                    Color.clear.frame(width: 30, height: 30)
                    ControllerButton(title: "Cir", active: buttons.contains("Circle"), size: 32)
                }
                ControllerButton(title: "X", active: buttons.contains("Cross"), size: 32)
            }
            .frame(width: 108, height: 108)
            Text("Face buttons")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(width: 108)
    }
}

private struct DPadMonitor: View {
    let buttons: Set<String>

    var body: some View {
        VStack(spacing: 6) {
            VStack(spacing: 4) {
                DPadButton(symbol: "chevron.up", active: buttons.contains("DpadUp"))
                HStack(spacing: 4) {
                    DPadButton(symbol: "chevron.left", active: buttons.contains("DpadLeft"))
                    Circle()
                        .fill(Color.secondary.opacity(0.16))
                        .frame(width: 30, height: 30)
                    DPadButton(symbol: "chevron.right", active: buttons.contains("DpadRight"))
                }
                DPadButton(symbol: "chevron.down", active: buttons.contains("DpadDown"))
            }
            .frame(width: 108, height: 108)
            Text("D-pad")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

private struct DPadButton: View {
    let symbol: String
    let active: Bool

    var body: some View {
        Circle()
            .fill(active ? Color.accentColor : Color.secondary.opacity(0.12))
            .overlay {
                Image(systemName: symbol)
                    .font(.caption.weight(.bold))
                    .foregroundStyle(active ? .white : .primary)
            }
            .frame(width: 32, height: 32)
    }
}

private struct TriggerAndBatteryView: View {
    let input: GamepadInput

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ShoulderButtonPair(buttons: Set(input.buttons))
            TriggerMeter(title: "L2", value: input.leftTrigger)
            TriggerMeter(title: "R2", value: input.rightTrigger)
            if let battery = input.batteryPercent {
                Label("\(battery)% \(input.batteryStatus.lowercased())", systemImage: "battery.75percent")
                    .font(.caption)
            }
        }
    }
}

private struct ShoulderButtonPair: View {
    let buttons: Set<String>

    var body: some View {
        HStack(spacing: 8) {
            ShoulderButton(title: "L1", active: buttons.contains("L1"))
            ShoulderButton(title: "R1", active: buttons.contains("R1"))
        }
        .frame(width: 130)
    }
}

private struct ShoulderButton: View {
    let title: String
    let active: Bool

    var body: some View {
        Capsule()
            .fill(active ? Color.accentColor : Color.secondary.opacity(0.12))
            .overlay {
                Text(title)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(active ? .white : .primary)
            }
            .frame(width: 61, height: 28)
    }
}

private struct TriggerMeter: View {
    let title: String
    let value: UInt8

    private var rawProgress: CGFloat {
        CGFloat(value) / CGFloat(UInt8.max)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(title)
                Spacer()
                Text("\(value)")
                    .monospacedDigit()
            }
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(Color.secondary.opacity(0.18))
                    Capsule()
                        .fill(Color.accentColor)
                        .frame(width: proxy.size.width * rawProgress)
                }
            }
            .frame(height: 9)
            .transaction { transaction in
                transaction.animation = nil
            }
        }
        .frame(width: 130)
        .accessibilityValue("\(value) of 255")
    }
}

private struct LightbarSwatch: View {
    let color: RGBColor

    var body: some View {
        RoundedRectangle(cornerRadius: 6)
            .fill(Color(red: Double(color.r) / 255, green: Double(color.g) / 255, blue: Double(color.b) / 255))
            .frame(height: 56)
            .overlay(alignment: .bottomLeading) {
                Text(String(format: "#%02X%02X%02X", color.r, color.g, color.b))
                    .font(.caption.monospaced())
                    .padding(8)
                    .foregroundStyle(.white)
                    .shadow(radius: 2)
            }
    }
}

private enum ControlRowMetrics {
    static let labelWidth: CGFloat = 160
    static let valueWidth: CGFloat = 64
    static let spacing: CGFloat = 12
    static let minimumSliderWidth: CGFloat = 220
    static let idealSliderWidth: CGFloat = 400
    static let maximumSliderWidth: CGFloat = 440
}

private struct ColorSlider: View {
    let label: String
    let value: Int
    let tint: Color
    let onChange: (Int) -> Void

    var body: some View {
        BufferedSlider(
            label: label,
            value: value,
            range: 0 ... 255,
            tint: tint,
            onCommit: onChange
        )
    }
}

private struct ValueSlider: View {
    let label: String
    let value: Int
    let range: ClosedRange<Int>
    var suffix = ""
    let onChange: (Int) -> Void

    var body: some View {
        BufferedSlider(
            label: label,
            value: value,
            range: range,
            tint: .accentColor,
            suffix: suffix,
            onCommit: onChange
        )
    }
}

/// A drag changes only local view state. Applying it once at the end avoids a
/// burst of full service snapshots and keeps every settings form responsive.
private struct BufferedSlider: View {
    let label: String
    let value: Int
    let range: ClosedRange<Int>
    let tint: Color
    var suffix = ""
    let onCommit: (Int) -> Void

    @State private var draftValue: Int?
    @State private var isEditing = false

    private var displayedValue: Int {
        (draftValue ?? value).clamped(to: range)
    }

    var body: some View {
        HStack(spacing: ControlRowMetrics.spacing) {
            Text(label)
                .frame(width: ControlRowMetrics.labelWidth, alignment: .leading)
            Spacer(minLength: ControlRowMetrics.spacing)
            Slider(
                value: Binding(
                    get: { Double(displayedValue) },
                    set: { draftValue = Int($0.rounded()).clamped(to: range) }
                ),
                in: Double(range.lowerBound) ... Double(range.upperBound),
                onEditingChanged: editingChanged
            )
            .tint(tint)
            .controlSize(.regular)
            .transaction { $0.animation = nil }
            .frame(
                minWidth: ControlRowMetrics.minimumSliderWidth,
                idealWidth: ControlRowMetrics.idealSliderWidth,
                maxWidth: ControlRowMetrics.maximumSliderWidth
            )
            .frame(height: 24)
            .layoutPriority(1)
            Text("\(displayedValue)\(suffix)")
                .monospacedDigit()
                .frame(width: ControlRowMetrics.valueWidth, alignment: .trailing)
        }
        .onAppear {
            draftValue = value.clamped(to: range)
        }
        .onChange(of: value) { updatedValue in
            guard !isEditing else {
                return
            }
            draftValue = updatedValue.clamped(to: range)
        }
    }

    private func editingChanged(_ editing: Bool) {
        isEditing = editing
        if editing {
            draftValue = value.clamped(to: range)
            return
        }

        let committedValue = displayedValue
        guard committedValue != value else {
            return
        }
        onCommit(committedValue)
    }
}

private struct StringPicker: View {
    let label: String
    let selection: String
    let options: [(String, String)]
    let onChange: (String) -> Void

    var body: some View {
        Picker(
            label,
            selection: Binding(
                get: { selection },
                set: onChange
            )
        ) {
            ForEach(options, id: \.1) { option in
                Text(option.0).tag(option.1)
            }
        }
    }
}

private struct MappingPicker: View {
    let source: String
    let selection: String
    let options: [(String, String)]
    let onChange: (String) -> Void

    private var selectedTitle: String {
        options.first(where: { $0.1 == selection })?.0 ?? selection
    }

    var body: some View {
        HStack(spacing: 12) {
            Text(source)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            Menu {
                // AppKit builds the menu when it opens, rather than keeping a
                // native picker and all of its option views alive per row.
                ForEach(options, id: \.1) { option in
                    Button(option.0) {
                        onChange(option.1)
                    }
                }
            } label: {
                Text(selectedTitle)
                    .frame(width: 176, alignment: .leading)
                    .lineLimit(1)
            }
            .menuStyle(.borderedButton)
            .controlSize(.small)
        }
        .frame(
            minHeight: MappingRowMetrics.height,
            maxHeight: MappingRowMetrics.height
        )
        .listRowInsets(
            EdgeInsets(
                top: 0,
                leading: MappingRowMetrics.horizontalInset,
                bottom: 0,
                trailing: MappingRowMetrics.horizontalInset
            )
        )
    }
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}

private struct AudioMeter: View {
    let low: UInt16
    let high: UInt16

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            AudioMeterRow(label: "Low", value: low)
            AudioMeterRow(label: "High", value: high)
        }
    }
}

private struct AudioMeterRow: View {
    let label: String
    let value: UInt16

    var body: some View {
        HStack(spacing: ControlRowMetrics.spacing) {
            Text(label)
                .frame(width: ControlRowMetrics.labelWidth, alignment: .leading)
            Spacer(minLength: ControlRowMetrics.spacing)
            ProgressView(value: Double(value), total: Double(UInt16.max))
                .frame(
                    minWidth: ControlRowMetrics.minimumSliderWidth,
                    idealWidth: ControlRowMetrics.idealSliderWidth,
                    maxWidth: ControlRowMetrics.maximumSliderWidth
                )
                .layoutPriority(1)
            Text("\(percentage)%")
                .monospacedDigit()
                .frame(width: ControlRowMetrics.valueWidth, alignment: .trailing)
        }
    }

    private var percentage: Int {
        Int((Double(value) / Double(UInt16.max) * 100).rounded())
    }
}

private struct HapticDemoGroup: View {
    let title: String
    let demos: [HapticDemoOption]
    let play: (HapticDemoOption) -> Void

    private var columns: [GridItem] {
        Array(repeating: GridItem(.flexible(minimum: 118, maximum: 148), spacing: 8), count: 4)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
            LazyVGrid(columns: columns, alignment: .leading, spacing: 8) {
                ForEach(demos, id: \.rawValue) { demo in
                    Button(demo.title) {
                        play(demo)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.regular)
                    .frame(maxWidth: .infinity)
                }
            }
        }
    }
}

private enum HapticDemoOption: String, CaseIterable {
    case click = "Click"
    case thump = "Thump"
    case buzz = "Buzz"
    case heartbeat = "Heartbeat"
    case sweep = "Sweep"
    case impact = "Impact"
    case tap = "Tap"
    case pulseTrain = "PulseTrain"

    static let impactDemos: [Self] = [.click, .tap, .impact, .thump]
    static let patternDemos: [Self] = [.buzz, .heartbeat, .sweep, .pulseTrain]

    var title: String {
        switch self {
        case .pulseTrain: return "Pulse Train"
        default: return rawValue
        }
    }
}

private enum TriggerPresetOption: String, CaseIterable {
    case off = "Off"
    case bow = "Bow"
    case machineGun = "MachineGun"
    case pistol = "Pistol"
    case rigid = "Rigid"
    case brake = "Brake"
    case pulse = "Pulse"
    case click = "Click"

    var title: String {
        switch self {
        case .machineGun: return "Machine Gun"
        default: return rawValue
        }
    }
}

private let buttonOptions: [(String, String)] = [
    ("Cross", "Cross"),
    ("Circle", "Circle"),
    ("Square", "Square"),
    ("Triangle", "Triangle"),
    ("L1", "L1"),
    ("R1", "R1"),
    ("L2", "L2"),
    ("R2", "R2"),
    ("Create", "Create"),
    ("Options", "Options"),
    ("L3", "L3"),
    ("R3", "R3"),
    ("PS", "Ps"),
    ("Touchpad", "Touchpad"),
    ("Mute", "Mute"),
    ("DPad Up", "DpadUp"),
    ("DPad Down", "DpadDown"),
    ("DPad Left", "DpadLeft"),
    ("DPad Right", "DpadRight"),
    ("Fn 1", "Fn1"),
    ("Fn 2", "Fn2"),
    ("Left paddle", "LeftPaddle"),
    ("Right paddle", "RightPaddle"),
]

private let keyboardKeyOptions: [(String, String)] = [
    ("Disabled", "Disabled"),
    ("Space", "Space"),
    ("Return", "Return"),
    ("Escape", "Escape"),
    ("Tab", "Tab"),
    ("Up arrow", "Up"),
    ("Down arrow", "Down"),
    ("Left arrow", "Left"),
    ("Right arrow", "Right"),
    ("W", "W"),
    ("A", "A"),
    ("S", "S"),
    ("D", "D"),
    ("Q", "Q"),
    ("E", "E"),
    ("R", "R"),
    ("F", "F"),
    ("Shift", "Shift"),
    ("Control", "Control"),
    ("Option", "Option"),
    ("1", "Key1"),
    ("2", "Key2"),
    ("3", "Key3"),
    ("4", "Key4"),
]

private func buttonTitle(_ button: String) -> String {
    buttonOptions.first(where: { $0.1 == button })?.0 ?? button
}
