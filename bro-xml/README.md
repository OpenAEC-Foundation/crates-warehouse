# bro-xml

`bro-xml` is een netwerk-vrije Rust-parser voor BRO XML-documenten van de typen CPT, BHR-GT en BHR-G. De parser bewaart onbekende extensievelden en originele referentiecodes, zodat nieuwere waarden niet verloren gaan. Bekende referentiecodes kunnen optioneel worden beschreven met `describe_reference_code`.

## Gebruik

Automatische typedetectie:

```rust
let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cpt-minimal.xml"));
let detected = bro_xml::detect(xml)?;
assert_eq!(detected.document_type, bro_xml::BroDocumentType::Cpt);
# Ok::<(), bro_xml::BroError>(())
```

Automatisch parsen:

```rust
let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cpt-minimal.xml"));
let document = bro_xml::parse(xml)?;
assert!(matches!(document, bro_xml::BroDocument::Cpt(_)));
# Ok::<(), bro_xml::BroError>(())
```

Een CPT direct parsen:

```rust
let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cpt-minimal.xml"));
let cpt = bro_xml::parse_cpt(xml)?;
assert!(!cpt.measurements.is_empty());
# Ok::<(), bro_xml::BroError>(())
```

Een geotechnisch booronderzoek direct parsen:

```rust
let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bhr-gt-minimal.xml"));
let borehole = bro_xml::parse_bhr_gt(xml)?;
assert_eq!(borehole.intervals.len(), 2);
# Ok::<(), bro_xml::BroError>(())
```

Een geologisch booronderzoek direct parsen:

```rust
let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bhr-g-minimal.xml"));
let borehole = bro_xml::parse_bhr_g(xml)?;
assert_eq!(borehole.intervals.len(), 2);
# Ok::<(), bro_xml::BroError>(())
```

De varianten `parse_with_options`, `parse_cpt_with_options`, `parse_bhr_gt_with_options` en `parse_bhr_g_with_options` accepteren `ParseOptions { retain_source: true }` om de volledige bron-XML in het resultaat te bewaren.

## Referentiecodes bijwerken

De ingecheckte tabellen maken normale builds reproduceerbaar en netwerk-vrij. Een maintainer kan ze handmatig opnieuw genereren via de officiële referentiecode-service:

```text
cargo run -p bro-reference-codegen -- bro-xml/src/reference_codes.rs
```

## Inspiratie en referenties

De API-ergonomie en objectdekking zijn mede geïnspireerd door [Bedrock's TypeScript BRO-XML parser](https://github.com/bedrock-engineer/bro-xml-parser-ts), in het bijzonder automatische typedetectie, ondersteuning voor CPT/BHR-GT/BHR-G en referentiecode-lookups. `bro-xml` is een onafhankelijke Rust-implementatie; het is geen port, binding of officiële samenwerking.
