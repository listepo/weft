import SwiftUI

/// The view for the `rating` kind of `catalog.json`. Weft has no SwiftUI form for a kind of the
/// project's own, so the generated `ReviewScreen` calls this one: its stored properties are the
/// kind's props in catalog order, and a prop a screen may leave out has a default.
struct RatingView: View {
    var value: Double
    var color: Color = .yellow

    var body: some View {
        HStack(spacing: 2) {
            ForEach(0..<5) { star in
                Image(systemName: Double(star) < value ? "star.fill" : "star")
                    .foregroundStyle(color)
            }
        }
    }
}
