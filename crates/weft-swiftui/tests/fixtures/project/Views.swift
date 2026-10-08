// The views an app writes for the kinds of its own catalog (`catalog.json`) and of the libraries
// it loads (`catalogs/acme-ui.catalog.json` in `examples/project`); the generated screens call them. Stored properties follow the order `generate` passes them in: props in
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

struct AcmeCardView<Content: View, Header: View, Footer: View>: View {
    var elevated: Bool = false
    @ViewBuilder var content: Content
    @ViewBuilder var header: Header
    @ViewBuilder var footer: Footer

    var body: some View {
        VStack(alignment: .leading) {
            header
            content
            footer
        }
        .padding()
        .shadow(radius: elevated ? 4 : 0)
    }
}

struct AcmeBadgeView<Content: View>: View {
    var pill: Bool = false
    @ViewBuilder var content: Content

    var body: some View {
        content
            .padding(.horizontal, 8)
            .background(Capsule().fill(.tint.opacity(pill ? 0.2 : 0)))
    }
}

struct AcmeButtonView<Content: View, Prefix: View, Suffix: View>: View {
    var variant: String = "default"
    var size: String = "medium"
    var disabled: Bool = false
    var href: String? = nil
    var onAcmeClick: () -> Void = {}
    var onAcmeFocus: () -> Void = {}
    @ViewBuilder var content: Content
    @ViewBuilder var prefix: Prefix
    @ViewBuilder var suffix: Suffix

    var body: some View {
        Button(action: onAcmeClick) {
            HStack {
                prefix
                content
                suffix
            }
        }
        .buttonStyle(.bordered)
        .controlSize(size == "small" ? .small : size == "large" ? .large : .regular)
        .disabled(disabled)
    }
}
