// The views an app writes for the kinds of its own catalog (`catalog.json`); the generated
// screens call them. Stored properties follow the order `generate` passes them in: props in
// catalog order, `state`, events, then content and slots as view builders.

import SwiftUI

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

struct PromoCardView<Content: View, HeaderActions: View, Footer: View>: View {
    var title: String
    var tone: String = "quiet"
    var pinned: Bool = false
    var count: Int = 1
    var accent: Color = .accentColor
    var gap: CGFloat = 8
    var note: Binding<String> = .constant("")
    var state: String = "idle"
    var onDismiss: () -> Void = {}
    var onOpenLink: () -> Void = {}
    @ViewBuilder var content: Content
    @ViewBuilder var headerActions: HeaderActions
    @ViewBuilder var footer: Footer

    var body: some View {
        VStack(alignment: .leading, spacing: gap) {
            HStack {
                Text(title).font(tone == "loud" ? .headline : .body)
                Spacer()
                headerActions
            }
            content
            TextField("Note", text: note)
            footer
        }
        .padding()
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(accent))
    }
}
