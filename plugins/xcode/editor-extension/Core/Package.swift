// swift-tools-version: 6.0
import PackageDescription

// What the Source Editor Extension does with a buffer, apart from the XcodeKit glue: running the
// bundled `weft` and shaping its answer. It lives in a package so `swift test` covers it without
// Xcode's UI or a signed extension.
let package = Package(
    name: "WeftEditorCore",
    platforms: [.macOS(.v14)],
    products: [.library(name: "WeftEditorCore", targets: ["WeftEditorCore"])],
    targets: [
        .target(name: "WeftEditorCore"),
        .testTarget(name: "WeftEditorCoreTests", dependencies: ["WeftEditorCore"]),
    ]
)
