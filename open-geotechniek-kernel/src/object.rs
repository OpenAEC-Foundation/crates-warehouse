/// A geotechnical object stored in a project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum GeotechnicalObject {
    /// A cone penetration test in the shared CPT domain model.
    Cpt(cpt_core::Cpt),
    /// A geotechnical borehole document.
    BhrGt(bro_xml::BhrGtDocument),
    /// A geological borehole document.
    BhrG(bro_xml::BhrGDocument),
}

/// The domain family of a [`GeotechnicalObject`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    /// A cone penetration test.
    Cpt,
    /// A geotechnical borehole investigation.
    BhrGt,
    /// A geological borehole investigation.
    BhrG,
}

impl GeotechnicalObject {
    /// Returns the stable identifier used as the project key.
    pub fn id(&self) -> &str {
        match self {
            Self::Cpt(cpt) => &cpt.id,
            Self::BhrGt(document) => &document.common.bro_id,
            Self::BhrG(document) => &document.common.bro_id,
        }
    }

    /// Returns the object's domain family.
    pub fn kind(&self) -> ObjectKind {
        match self {
            Self::Cpt(_) => ObjectKind::Cpt,
            Self::BhrGt(_) => ObjectKind::BhrGt,
            Self::BhrG(_) => ObjectKind::BhrG,
        }
    }
}
