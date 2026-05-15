# cpt-core

CPT (Cone Penetration Test) domain library for the OpenAEC ecosystem.

## Features
- GEF 1.x parser (Dutch Geotechnical Exchange Format)
- BRO-XML parser (Dutch Basisregistratie Ondergrond CPT_O / CPT_O_DP)
- Robertson 1990 SBT classification
- Layer detection (consecutive same-zone grouping)
- RD ↔ WGS84 coordinate transformation (Bessel 1841 + 7-param Helmert)
- SVG plot rendering (NEN-EN-ISO 22476-1 layout)
- Report builder producing `openaec_core::ReportData`

## License
MIT
