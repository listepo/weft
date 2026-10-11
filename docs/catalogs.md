# Catalogs

Known catalogs that extend the core catalog `weft-core`, and the prefixes they take. The list is kept by the maintainer of this repository. To add a catalog, open a pull request that adds a row; [Publishing a catalog](publishing-catalogs.md) explains how to make and publish one.

The list is a courtesy, not an authority. Nothing stops two catalogs from using one prefix, but a project that loads both gets `W711`. Before you pick a prefix, check that it is not listed here or reserved.

Outside this list, published catalogs carry the npm keyword `weft-catalog`: `npm search keywords:weft-catalog`.

## Listed catalogs

| Catalog | Prefix | Package | Source | Maintainer |
| --- | --- | --- | --- | --- |

No catalogs are listed yet. `acme-ui` in `examples/project` is an example and does not take the `acme` prefix.

## Reserved prefixes

A library cannot use these ([SPEC §5](../SPEC.md#5-catalog), `W712`):

- `x`, which marks extensions (`x-<vendor>-<name>`), and `weft`.
- The name or first segment of a core component of `weft-core` 0.2: `alert`, `button`, `cell`, `checkbox`, `color`, `column`, `combobox`, `date`, `dialog`, `field`, `form`, `grid`, `heading`, `image`, `item`, `link`, `list`, `menu`, `model`, `option`, `radio`, `row`, `screen`, `section`, `segment`, `segmented`, `select`, `slider`, `stack`, `stepper`, `switch`, `tab`, `table`, `tabs`, `text`.
