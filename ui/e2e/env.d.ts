// The e2e project does not depend on @types/node; the config only reads environment
// variables, so declare just that.
declare const process: { env: Record<string, string | undefined> };
