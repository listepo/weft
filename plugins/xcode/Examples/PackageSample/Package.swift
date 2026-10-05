// swift-tools-version: 6.0
import PackageDescription

// A library whose screens are written in Weft: `Sources/Screens/login.weft` becomes SwiftUI at
// build time, so `LoginScreen` exists without a hand-written or committed Swift file for it.
let package = Package(
    name: "PackageSample",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [.library(name: "Screens", targets: ["Screens"])],
    dependencies: [.package(path: "../..")],
    targets: [
        .target(
            name: "Screens",
            plugins: [.plugin(name: "WeftBuildTool", package: "xcode")]
        )
    ]
)
