// swift-tools-version: 6.0
import PackageDescription

// A library whose screens are written in Weft: `login.weft` and `review.weft` in `Sources/Screens`
// become SwiftUI at build time, with one `WeftTokens.swift` for both, so `LoginScreen` and
// `ReviewScreen` exist without a hand-written or committed Swift file for them. `review.weft` uses
// `rating`, a kind of the project's `catalog.json` that the app writes (`RatingView.swift`).
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
