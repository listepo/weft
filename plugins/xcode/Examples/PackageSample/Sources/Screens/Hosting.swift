import SwiftUI

/// Uses the types the build tool plugin generates from `login.weft` and `review.weft`, so the
/// target only compiles when the generated files, and the one `WeftTokens.swift` both read, are
/// part of it.
public struct LoginHost: View {
    @State private var model = LoginModel()

    public init() {}

    public var body: some View {
        LoginScreen(model: model) { event in
            print("login fired \(event.action.rawValue) from \(event.id)")
        }
    }
}

public struct ReviewHost: View {
    @State private var model = ReviewModel()

    public init() {}

    public var body: some View {
        ReviewScreen(model: model) { event in
            print("review fired \(event.action.rawValue) from \(event.id)")
        }
    }
}
