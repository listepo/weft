import SwiftUI

/// Uses the types the build tool plugin generates from `login.weft`, so the target only compiles
/// when the generated file is part of it.
public struct LoginHost: View {
    @State private var model = LoginModel()

    public init() {}

    public var body: some View {
        LoginScreen(model: model) { event in
            print("login fired \(event.action.rawValue) from \(event.id)")
        }
    }
}
