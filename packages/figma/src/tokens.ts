// Figma names variables with `/` between groups; Weft token paths use `.`.
export const variableName = (path: string): string => path.replaceAll(".", "/");
export const tokenPathOf = (name: string): string => name.replaceAll("/", ".");
