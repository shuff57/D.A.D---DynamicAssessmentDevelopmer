// Stands in for wasm-feature-detect so the loader always picks the asyncify build, which runs in every
// current browser; only that one .wasm (18 MB, about 5 MB compressed) is shipped.
export const jspi = async () => false;
export default { jspi };
