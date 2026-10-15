// One window's document over the `ws_*` C ABI of weft-studio-ffi. Rust owns the document and the
// screen; this class only turns its calls, its state JSON and its events into Swift values.

import CWeftStudio
import Foundation
import Observation
import SlintBindings

/// A failed call: the first line says what failed, the rest are the diagnostics behind it.
struct StudioError: Error, Equatable {
    let message: String

    var summary: String { String(message.prefix { $0 != "\n" }) }

    var details: String {
        message.firstIndex(of: "\n").map { String(message[message.index(after: $0)...]) } ?? ""
    }

    static func last() -> StudioError {
        StudioError(message: ws_last_error().map { String(cString: $0) } ?? "unknown error")
    }
}

/// The fields of `ws_state_json` the shell shows.
struct DocumentState: Decodable, Equatable {
    let v: UInt32
    let path: String
    let title: String
    let screen: String?
}

@MainActor
@Observable
final class StudioDocument {
    private(set) var state: DocumentState?
    /// The screen, or nil when no document is open or its screen was refused.
    private(set) var surface: SlintHost?
    /// The error the window shows; the alert clears it.
    var error: StudioError?

    @ObservationIgnored private let editor: OpaquePointer?
    @ObservationIgnored private let id: Int

    // The C callback cannot capture a document, and a surface may outlive its window, so events
    // carry an id rather than a pointer, and a closed document's events find nothing.
    private static var live: [Int: Weak] = [:]
    private static var nextID = 1

    private struct Weak {
        weak var document: StudioDocument?
    }

    init() {
        id = Self.nextID
        Self.nextID += 1
        guard ws_api_version() == UInt32(WS_API_VERSION) else {
            editor = nil
            error = StudioError(message: "weft-studio-ffi speaks API \(ws_api_version()), this app \(WS_API_VERSION)")
            return
        }
        editor = ws_editor_new({ userData, json in
            guard let json else { return }
            let text = String(cString: json)
            let id = Int(bitPattern: userData)
            // Rust raises events synchronously inside the `ws_*` and `sb_host_*` calls, which
            // only ever run on the main actor.
            MainActor.assumeIsolated { StudioDocument.live[id]?.document?.handle(text) }
        }, UnsafeMutableRawPointer(bitPattern: id))
        Self.live[id] = Weak(document: self)
    }

    isolated deinit {
        Self.live[id] = nil
        ws_editor_free(editor)
    }

    /// Opens `url` and makes its surface. A refused screen stays open without one and says why.
    @discardableResult
    func open(_ url: URL) -> Bool {
        guard call({ ws_open(editor, url.path) }) else { return false }
        surface = nil
        do {
            surface = try SlintHost(adopting: try surfaceHandle())
        } catch let error as StudioError {
            self.error = error
        } catch {
            self.error = StudioError(message: "\(error)")
        }
        return true
    }

    /// Writes the canonical markup back to the document's path.
    @discardableResult
    func save() -> Bool {
        state != nil && call { ws_save(editor, nil) }
    }

    /// Writes the document where the picker says; that path becomes the document's.
    @discardableResult
    func saveAs(using picker: any FilePicking) -> Bool {
        guard let state, let url = picker.pickSaveLocation(suggestedName: state.title) else { return false }
        return call { ws_save(editor, url.path) }
    }

    /// The host is made at one pixel: `SlintNSView` resizes it to the view.
    private func surfaceHandle() throws(StudioError) -> OpaquePointer {
        guard let handle = ws_surface_new(editor, 1, 1, 1) else { throw StudioError.last() }
        return handle
    }

    private func call(_ body: () -> Bool) -> Bool {
        if body() { return true }
        error = StudioError.last()
        return false
    }

    private struct Event: Decodable {
        let v: UInt32
        let type: String
    }

    private func handle(_ json: String) {
        guard let event = try? JSONDecoder().decode(Event.self, from: Data(json.utf8)),
              event.v == UInt32(WS_API_VERSION) else { return }
        // M1 is a live preview: the screen's `action` events change nothing.
        if event.type == "changed" { refresh() }
    }

    private func refresh() {
        guard let text = ws_state_json(editor) else {
            error = StudioError.last()
            return
        }
        defer { ws_string_free(text) }
        do {
            state = try JSONDecoder().decode(DocumentState.self, from: Data(String(cString: text).utf8))
        } catch {
            self.error = StudioError(message: "unreadable editor state: \(error)")
        }
    }
}
