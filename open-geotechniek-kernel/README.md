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

## Project-file compatibility

`load_project_text()` and `load_project_file()` accept existing `.ifcgis` and
IFCX projects in memory. The kernel updates typed project metadata, CPTs, and
borehole objects while retaining drawing, GIS, deliverable, calculation, and
other compatibility sections from the loaded project template. Opaque legacy
borehole JSON is preserved unchanged. A borehole with retained BRO source XML
is promoted to a typed object and written back using the established `id`,
`position`, `final_depth`, `layers`, and `metadata` shape. Promotion requires
the wrapper ID and parsed XML ID to agree; mismatched content remains opaque.
Opaque string IDs still reserve their project identity, so later imports cannot
silently shadow them. Direct BHR imports retain their source XML to remain typed
across project-file round trips.

`to_project_file()` and `to_project_text()` return in-memory values. Reading
and writing paths remains the responsibility of application adapters.

## Imports and CPT layers

`import_bro()` detects CPT, BHR-GT, and BHR-G XML. BRO CPT measurements are
converted loss-aware into `cpt_core::Cpt`: the supported measurement channels
are mapped directly, vertical depth is calculated only when a vertical offset
exists, and remaining common metadata is retained in `Metadata::extra`. Only
coordinates explicitly identified as EPSG:28992 populate the RD-specific typed
position. Coordinates in another CRS remain available losslessly as
`position_crs`, `position_x`, and `position_y` in `Metadata::extra`.

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
