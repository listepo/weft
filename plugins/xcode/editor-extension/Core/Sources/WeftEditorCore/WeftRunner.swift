import Foundation

/// Runs the `weft` executable on text from the editor. The extension cannot read the user's
/// files (it is sandboxed and XcodeKit gives it no path), so the text goes through a file in the
/// process's own temporary directory.
public struct WeftRunner: Sendable {
    public struct Output: Sendable, Equatable {
        public var status: Int32
        public var standardOutput: String
        /// Diagnostics, losses and notes. Both streams have the temporary file name removed.
        public var standardError: String
    }

    public enum Failure: Error, Equatable, LocalizedError {
        case cannotStart(String)
        case timedOut

        public var errorDescription: String? {
            switch self {
            case .cannotStart(let reason): "weft could not be started: \(reason)"
            case .timedOut: "weft took too long and was stopped"
            }
        }
    }

    public let executable: URL
    /// The core catalog, so `validate` checks the vocabulary and not only the syntax.
    public let catalog: URL?
    public let timeout: TimeInterval

    public init(executable: URL, catalog: URL? = nil, timeout: TimeInterval = 30) {
        self.executable = executable
        self.catalog = catalog
        self.timeout = timeout
    }

    /// `weft <subcommand> <file> --no-project <extra>` on `text`, saved as `fileName`. `--no-project`
    /// because the temporary file has no project around it; the editor has none to offer.
    public func run(_ subcommand: String, text: String, fileName: String, extra: [String] = []) throws -> Output {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("weft-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent(fileName)
        try text.write(to: file, atomically: true, encoding: .utf8)

        // Output goes to files, not pipes: a pipe the child fills while we wait would deadlock.
        let outURL = directory.appendingPathComponent("stdout")
        let errURL = directory.appendingPathComponent("stderr")
        FileManager.default.createFile(atPath: outURL.path, contents: nil)
        FileManager.default.createFile(atPath: errURL.path, contents: nil)
        let process = Process()
        process.executableURL = executable
        process.arguments = [subcommand, file.path, "--no-project"] + extra
        process.standardOutput = try FileHandle(forWritingTo: outURL)
        process.standardError = try FileHandle(forWritingTo: errURL)
        process.standardInput = FileHandle.nullDevice
        do {
            try process.run()
        } catch {
            throw Failure.cannotStart(error.localizedDescription)
        }
        let watchdog = DispatchWorkItem { if process.isRunning { process.terminate() } }
        DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: watchdog)
        process.waitUntilExit()
        watchdog.cancel()
        if process.terminationReason == .uncaughtSignal { throw Failure.timedOut }

        // `validate` prints its diagnostics to standard output, the others to standard error.
        let read = { (url: URL) in
            String(decoding: (try? Data(contentsOf: url)) ?? Data(), as: UTF8.self)
        }
        let withoutPath = { (text: String) in
            text.replacingOccurrences(of: file.path + ":", with: "").replacingOccurrences(of: file.path, with: fileName)
        }
        return Output(
            status: process.terminationStatus, standardOutput: withoutPath(read(outURL)),
            standardError: withoutPath(read(errURL)))
    }
}
