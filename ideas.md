# Ideas

- Renderers for platforms other than the web.
- Publish the repository.
- Check the console blocks of the `docs/` guide in CI, so a page cannot drift from the commands it shows.
- SwiftUI context for screens that place fragments: the importer's source check fails on them, because the reader invents ids for the expanded instances, so their context does not read back. The generator could write instance ids the reader keeps.
- WinUI screenshots (after T105): run the generated pages in a small WinUI app on the Windows runner and compare screenshots with the other targets, as T36 and T74 do.
- WinUI user controls for catalog-extension kinds (after T105): `promo-card` → `local:PromoCardControl` the app writes, as SwiftUI calls `PromoCardView`.
