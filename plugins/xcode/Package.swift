// swift-tools-version: 6.0
import PackageDescription

// Weft in SwiftPM and Xcode: a build tool plugin that turns every `.weft` file of a target into
// SwiftUI while it builds, and a command plugin that converts a screen or a view once.
let package = Package(
    name: "WeftXcode",
    // Plugins run on the machine that builds, which must be Apple Silicon; the code they generate
    // targets iOS 17 and macOS 14.
    platforms: [.macOS(.v14)],
    products: [
        .plugin(name: "WeftBuildTool", targets: ["WeftBuildToolPlugin"]),
        .plugin(name: "WeftCommands", targets: ["WeftCommandPlugin"]),
    ],
    targets: [
        // Built by `moon run xcode-plugin:bundle`. A release will point at a GitHub Releases URL
        // with a checksum instead, once the repository has a remote.
        .binaryTarget(name: "weft", path: "Artifacts/weft.artifactbundle"),
        .plugin(
            name: "WeftBuildToolPlugin",
            capability: .buildTool(),
            dependencies: ["weft"]
        ),
        .plugin(
            name: "WeftCommandPlugin",
            capability: .command(
                intent: .custom(
                    verb: "weft",
                    description: "Convert a Weft screen to SwiftUI (export) or a SwiftUI view to a Weft screen (import)"
                ),
                permissions: [
                    .writeToPackageDirectory(
                        reason: "Writes the generated SwiftUI file or the imported .weft screen into the package"
                    )
                ]
            ),
            dependencies: ["weft"]
        ),
    ]
)
