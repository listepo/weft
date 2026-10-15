// WeftStudio for macOS (T69.1): one window per `.weft` document, the File menu, and the screen on
// slint-bindings' `SlintNSView`. The window's value is the document's URL, so opening a file that
// is already open brings its window forward instead of making a second one.

import SlintBindings
import SwiftUI

@main
struct WeftStudioApp: App {
    private let picker = Pickers.make(environment: ProcessInfo.processInfo.environment)

    var body: some Scene {
        WindowGroup(for: URL.self) { $url in
            DocumentWindow(url: $url)
        }
        .defaultSize(width: 480, height: 800)
        .commands { FileCommands(picker: picker) }
    }
}

/// `WeftStudio -WeftStudioScreen <path>` opens the screen in the first window. A bare path would
/// not do: AppKit takes it as a request to open a document, and then makes no untitled window,
/// while SwiftUI ignores that request. A `-key value` pair lands in the argument domain instead.
enum LaunchOptions {
    static let screenKey = "WeftStudioScreen"

    static func screen(in defaults: UserDefaults) -> URL? {
        defaults.string(forKey: screenKey).map { URL(filePath: $0) }
    }

    @MainActor private static var taken = false

    /// The launch screen, once: later empty windows stay empty.
    @MainActor static func take() -> URL? {
        guard !taken else { return nil }
        taken = true
        return screen(in: .standard)
    }
}

struct FileCommands: Commands {
    let picker: any FilePicking
    @Environment(\.openWindow) private var openWindow
    @FocusedValue(StudioDocument.self) private var document

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Open…") {
                if let url = picker.pickFileToOpen() { openWindow(value: url) }
            }
            .keyboardShortcut("o")
        }
        CommandGroup(replacing: .saveItem) {
            Button("Save") { document?.save() }
                .keyboardShortcut("s")
                .disabled(document?.state == nil)
            Button("Save As…") { document?.saveAs(using: picker) }
                .keyboardShortcut("s", modifiers: [.command, .shift])
                .disabled(document?.state == nil)
        }
    }
}

struct DocumentWindow: View {
    @Binding var url: URL?
    @State private var document = StudioDocument()

    var body: some View {
        content
            .frame(minWidth: 320, minHeight: 320)
            .navigationTitle(document.state?.title ?? "WeftStudio")
            .focusedSceneValue(document)
            .alert(
                document.error?.summary ?? "",
                isPresented: Binding(get: { document.error != nil }, set: { if !$0 { document.error = nil } }),
                presenting: document.error
            ) { _ in
                Button("OK") {}
            } message: { error in
                Text(error.details)
            }
            .onAppear {
                if url == nil { url = LaunchOptions.take() }
            }
            .task(id: url) {
                if let url, document.state?.path != url.path { document.open(url) }
            }
            // After Save As the window stands for the new file, so opening that file finds it.
            .onChange(of: document.state?.path) { _, path in
                if let path { url = URL(filePath: path) }
            }
    }

    @ViewBuilder private var content: some View {
        if let surface = document.surface {
            SurfaceView(host: surface).id(ObjectIdentifier(surface))
        } else {
            Text(document.state == nil ? "Open a Weft screen with File › Open… (⌘O)" : "This screen cannot be shown.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

/// slint-bindings' view over the document's host, on the CPU path that `ws_surface_new` draws
/// on. The surface is invisible to VoiceOver until slint-bindings' T14, so it is one image to
/// accessibility, which is how the UI test finds it.
struct SurfaceView: NSViewRepresentable {
    let host: SlintHost

    func makeNSView(context: Context) -> SlintNSView {
        let view = SlintNSView(frame: .zero, host: host)
        view.setAccessibilityElement(true)
        view.setAccessibilityRole(.image)
        view.setAccessibilityIdentifier("surface")
        return view
    }

    func updateNSView(_ view: SlintNSView, context: Context) {}
}
