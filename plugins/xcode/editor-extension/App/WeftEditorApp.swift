import AppKit
import SwiftUI
import WeftEditorCore

@main
struct WeftEditorApp: App {
    init() {
        // `WeftEditor --self-test` runs the extension's conversion inside this app's sandbox and
        // exits, so a build can prove the embedded weft starts under the sandbox without Xcode's UI.
        if CommandLine.arguments.contains("--self-test") {
            exit(SelfTest.run())
        }
    }

    var body: some Scene {
        WindowGroup {
            VStack(alignment: .leading, spacing: 12) {
                Text("Weft for Xcode").font(.title)
                Text("This app carries the Weft source editor extension. Turn it on in System Settings > General > Login Items & Extensions > Xcode Source Editor, then restart Xcode. The commands are in Xcode's Editor menu.")
                    .fixedSize(horizontal: false, vertical: true)
                Button("Open Extension Settings") {
                    if let url = URL(string: "x-apple.systempreferences:com.apple.ExtensionsPreferences") {
                        NSWorkspace.shared.open(url)
                    }
                }
            }
            .padding(24)
            .frame(width: 460)
        }
    }
}

enum SelfTest {
    static let screen = "<screen id=\"hello\" weft=\"0.1\">\n  <text id=\"greeting\">Hello</text>\n</screen>\n"

    static func run() -> Int32 {
        let plugIns = Bundle.main.builtInPlugInsURL?.appendingPathComponent("WeftEditorExtension.appex")
        guard let weft = plugIns.flatMap(Bundle.init(url:))?.weftExecutable else {
            print("self-test failed: no weft in the extension bundle")
            return 1
        }
        let editor = WeftEditor(runner: WeftRunner(executable: weft))
        let outcome = editor.perform(.generateSwiftUI, on: Buffer(lines: [screen], isSwift: false))
        guard !outcome.isError, outcome.clipboard?.contains("struct HelloScreen: View") == true else {
            print("self-test failed: \(outcome.message)")
            return 1
        }
        // The container id is set only for a sandboxed process, which is the point of the test.
        guard ProcessInfo.processInfo.environment["APP_SANDBOX_CONTAINER_ID"] != nil else {
            print("self-test failed: the app is not sandboxed")
            return 1
        }
        print("self-test ok (sandboxed)")
        return 0
    }
}
