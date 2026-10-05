import SwiftUI

@main
struct XcodeSampleApp: App {
    // `LoginModel` and `LoginScreen` come from `login.weft`, generated at build time.
    @State private var model = LoginModel()

    var body: some Scene {
        WindowGroup {
            LoginScreen(model: model)
        }
    }
}
