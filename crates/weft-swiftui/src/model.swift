
// MARK: - Weft 3D model
// The `model` element (SPEC §9): the bundled USDZ file where the platform can show one, the still
// image while it loads, where it cannot, and whenever the process sets `WEFT_STILL_MODELS`, which
// the screenshot host does so that images are deterministic. Paths are the document's literals:
// a file name in the app bundle, or an `https` URL that only the still can use.

fileprivate struct WeftModel: View {
    var usdz: String? = nil
    var fallback: String

    var body: some View {
        ZStack {
            weftStill(fallback)
            if ProcessInfo.processInfo.environment["WEFT_STILL_MODELS"] == nil,
               let usdz, let url = weftBundled(usdz) {
                weftLive(url)
            }
        }
        .aspectRatio(4.0 / 3.0, contentMode: .fit)
        .accessibilityElement(children: .ignore)
        .accessibilityAddTraits(.isImage)
    }
}

@ViewBuilder
fileprivate func weftLive(_ url: URL) -> some View {
    #if os(visionOS)
    Model3D(url: url)
    #else
    if #available(iOS 18, macOS 15, *) {
        WeftRealityModel(url: url)
    }
    #endif
}

#if !os(visionOS)
@available(iOS 18, macOS 15, *)
fileprivate struct WeftRealityModel: View {
    let url: URL

    var body: some View {
        RealityView { content in
            content.camera = .virtual
            if let entity = try? await Entity(contentsOf: url) { content.add(entity) }
        }
    }
}
#endif

fileprivate func weftBundled(_ path: String) -> URL? {
    guard URL(string: path)?.scheme == nil else { return nil }
    let name = (path as NSString).lastPathComponent as NSString
    return Bundle.main.url(forResource: name.deletingPathExtension, withExtension: name.pathExtension)
}

@ViewBuilder
fileprivate func weftStill(_ path: String) -> some View {
    if let url = URL(string: path), url.scheme?.lowercased() == "https" {
        AsyncImage(url: url) { $0.resizable().scaledToFit() } placeholder: { Color.clear }
    } else if let url = weftBundled(path), let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) {
        Image(decorative: image, scale: 1).resizable().scaledToFit()
    } else {
        Color.clear
    }
}
