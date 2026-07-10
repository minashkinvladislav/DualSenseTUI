import SwiftUI

@main
struct DualSenseApp: App {
    @StateObject private var service = CoreService()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(service)
                .frame(minWidth: 980, minHeight: 680)
                .onAppear {
                    service.start()
                }
        }
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Refresh Controller") {
                    service.refresh()
                }
                .keyboardShortcut("r", modifiers: [.command])

                Button("Save Controller Profile") {
                    service.saveProfile()
                }
                .keyboardShortcut("s", modifiers: [.command])
            }
        }
    }
}
