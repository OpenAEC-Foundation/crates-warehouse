# Open Geotechniek Kernel

`open-geotechniek-kernel` is the in-memory project façade for geotechnical
applications. It combines typed BRO documents from `bro-xml` with the shared
CPT domain model from `cpt-core` without depending on a UI, web framework,
filesystem, or network client.

## Content boundary

Callers read or receive content themselves and pass it to the kernel together
with a source label. The kernel never opens the source label as a path. Results
are returned as typed in-memory values, so the same façade can be used by
desktop, command-line, web, and service adapters.

## Project operations

`GeotechnicalProject` stores objects by their stable identifier in a `BTreeMap`.
Consequently `objects()` and `cpts()` have deterministic ordering. `get()` and
`remove()` implement object CRUD; `metadata()` and `set_metadata()` manage the
project-level description. Normal imports reject duplicate identifiers.
`merge_from()` makes replacement behavior explicit through `DuplicatePolicy`.

## Imports and CPT layers

`import_bro()` detects CPT, BHR-GT, and BHR-G XML. BRO CPT measurements are
converted loss-aware into `cpt_core::Cpt`: the supported measurement channels
are mapped directly, vertical depth is calculated only when a vertical offset
exists, and remaining common metadata is retained in `Metadata::extra`.

`import_cpt()` accepts content supported by `cpt-core`. A source label ending in
`.ifcgeo` selects the IfcGeo reader; other labels use automatic content
detection. `detect_cpt_layers()` delegates CPT layer classification to
`cpt-core` and rejects non-CPT objects clearly.

```rust
use open_geotechniek_kernel::{GeotechnicalProject, ProjectMetadata};

let mut project = GeotechnicalProject::new(ProjectMetadata::default());
let snapshot = r#"{
    "id": "CPT-1",
    "metadata": { "source_file": "original.ifcgeo" },
    "position": null,
    "points": []
}"#;

project.import_cpt(snapshot, "CPT-1.ifcgeo")?;
assert_eq!(project.cpts().count(), 1);
assert!(project.detect_cpt_layers("CPT-1")?.is_empty());
# Ok::<(), open_geotechniek_kernel::KernelError>(())
```
