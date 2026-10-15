// Where the paths to open and save come from. The app asks the system panels; the UI test cannot
// drive those reliably, so the launch environment can name the answers instead.

import AppKit
import UniformTypeIdentifiers

@MainActor
protocol FilePicking {
    /// A `.weft` file to open, or nil when the user cancels.
    func pickFileToOpen() -> URL?
    /// Where to save, starting from `suggestedName`, or nil when the user cancels.
    func pickSaveLocation(suggestedName: String) -> URL?
}

extension UTType {
    /// Exported by WeftEditor, the Source Editor Extension's host app; this app imports it.
    static let weftScreen = UTType(importedAs: "dev.weft.screen", conformingTo: .text)
}

struct PanelPicker: FilePicking {
    func pickFileToOpen() -> URL? {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.weftScreen]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        return panel.runModal() == .OK ? panel.url : nil
    }

    func pickSaveLocation(suggestedName: String) -> URL? {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.weftScreen]
        panel.nameFieldStringValue = suggestedName
        return panel.runModal() == .OK ? panel.url : nil
    }
}

/// Fixed answers, so a test can open and save without the panels.
struct FixedPicker: FilePicking {
    var open: URL?
    var save: URL?

    func pickFileToOpen() -> URL? { open }
    func pickSaveLocation(suggestedName: String) -> URL? { save }
}

enum Pickers {
    static let openKey = "WEFT_STUDIO_OPEN_PATH"
    static let saveKey = "WEFT_STUDIO_SAVE_PATH"

    /// The panels, unless the environment names either answer.
    static func make(environment: [String: String]) -> any FilePicking {
        let open = environment[openKey].map { URL(filePath: $0) }
        let save = environment[saveKey].map { URL(filePath: $0) }
        if open == nil && save == nil { return PanelPicker() }
        return FixedPicker(open: open, save: save)
    }
}
