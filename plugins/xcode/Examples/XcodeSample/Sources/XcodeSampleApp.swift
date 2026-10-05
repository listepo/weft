import SwiftUI

@main
struct XcodeSampleApp: App {
    // `LoginModel`, `LoginScreen`, `ReviewModel` and `ReviewScreen` come from `login.weft` and
    // `review.weft`, generated at build time with the `WeftTokens` both read.
    @State private var login = LoginModel()
    @State private var review = ReviewModel()

    var body: some Scene {
        WindowGroup {
            TabView {
                LoginScreen(model: login).tabItem { Text("Login") }
                ReviewScreen(model: review).tabItem { Text("Review") }
            }
        }
    }
}
