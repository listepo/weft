
// MARK: - Weft reflowing grid

/// A grid with `min-column-width` (SPEC §5.1): `columns` columns while each can be `minWidth`
/// wide, fewer below that, down to one, judged by the grid's own width as on the web. Rows are as
/// tall as their tallest cell, and every cell is offered the row's height, as CSS grid stretches.
fileprivate struct WeftColumns: Layout {
    var columns: Int
    var minWidth: CGFloat
    /// Between columns and between rows, as `gap` is. A grid without one keeps the default
    /// spacing `LazyVGrid` gives the grids that do not reflow.
    var spacing: CGFloat = 8

    private func count(_ width: CGFloat?) -> Int {
        let most = max(1, columns)
        guard let width, width.isFinite else { return most }
        let fit = Int(((width + spacing) / (max(minWidth, 0) + spacing)).rounded(.down))
        return min(most, max(1, fit))
    }

    /// A column's width, or `nil` when the grid was offered no width to share.
    private func cell(_ width: CGFloat?, _ n: Int) -> CGFloat? {
        guard let width, width.isFinite else { return nil }
        return max(0, (width - spacing * CGFloat(n - 1)) / CGFloat(n))
    }

    private func rows(_ subviews: Subviews, _ n: Int, _ cell: CGFloat?) -> [CGFloat] {
        stride(from: 0, to: subviews.count, by: n).map { start in
            subviews[start..<min(start + n, subviews.count)]
                .map { $0.sizeThatFits(ProposedViewSize(width: cell, height: nil)).height }
                .max() ?? 0
        }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let n = count(proposal.width)
        let w = cell(proposal.width, n)
        let heights = rows(subviews, n, w)
        let height = heights.reduce(0, +) + spacing * CGFloat(max(0, heights.count - 1))
        if let width = proposal.width, width.isFinite {
            return CGSize(width: width, height: height)
        }
        let widest = subviews.map { $0.sizeThatFits(.unspecified).width }.max() ?? 0
        return CGSize(width: widest * CGFloat(n) + spacing * CGFloat(n - 1), height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let n = count(bounds.width)
        let w = cell(bounds.width, n) ?? 0
        var y = bounds.minY
        for (row, height) in rows(subviews, n, w).enumerated() {
            for column in 0..<n where row * n + column < subviews.count {
                let x = bounds.minX + CGFloat(column) * (w + spacing)
                subviews[row * n + column].place(
                    at: CGPoint(x: x, y: y),
                    anchor: .topLeading,
                    proposal: ProposedViewSize(width: w, height: height)
                )
            }
            y += height + spacing
        }
    }
}

/// One cell of `WeftColumns`: a layout sees a section's header and content as two views.
fileprivate struct WeftCell<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading) { content }
    }
}
