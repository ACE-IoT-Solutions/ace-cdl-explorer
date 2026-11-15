`graph.json` is a small project fixture with a weighted Add, a legacy Switch port
order and a nested composite. `old-export.jsonld` is its output from the original
TypeScript `exportCxf`, before Rust migration. It intentionally retains the old
interface-only class headers. The regression test imports this saved file and
compares executable outputs against the original project for both Switch states.
