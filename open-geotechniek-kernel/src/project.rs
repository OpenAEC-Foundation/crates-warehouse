use std::collections::BTreeMap;

use crate::{import, GeotechnicalObject, KernelError};

/// Descriptive metadata belonging to a project rather than an individual object.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectMetadata {
    /// Human-readable project title.
    pub title: String,
    /// Client name.
    pub client: String,
    /// Human-readable project location.
    pub location: String,
    /// Project number used by the caller.
    pub project_number: String,
    /// Author or responsible editor.
    pub author: String,
    /// Project date, when known.
    pub date: Option<chrono::NaiveDate>,
}

/// An in-memory geotechnical project with deterministically ordered objects.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeotechnicalProject {
    pub(crate) metadata: ProjectMetadata,
    pub(crate) objects: BTreeMap<String, GeotechnicalObject>,
    #[serde(skip)]
    pub(crate) project_template: Option<cpt_core::ifcgis::ProjectFile>,
    #[serde(skip)]
    pub(crate) compatibility_bores: Vec<serde_json::Value>,
}

/// Policy used when merging objects with identifiers already in the target project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicatePolicy {
    /// Abort the merge without inserting any objects.
    Reject,
    /// Replace existing objects with incoming objects sharing their identifiers.
    Replace,
}

impl GeotechnicalProject {
    /// Creates an empty project with the supplied metadata.
    pub fn new(metadata: ProjectMetadata) -> Self {
        Self {
            metadata,
            objects: BTreeMap::new(),
            project_template: None,
            compatibility_bores: Vec::new(),
        }
    }

    /// Parses BRO XML content and inserts it while rejecting duplicate identifiers.
    pub fn import_bro(
        &mut self,
        xml: &str,
        source_file: &str,
    ) -> Result<GeotechnicalObject, KernelError> {
        let object = import::parse_bro(xml, source_file)?;
        self.insert_rejecting_duplicates(object)
    }

    /// Parses GEF, BRO CPT XML, or IfcGeo content and inserts the resulting CPT.
    pub fn import_cpt(
        &mut self,
        content: &str,
        source_file: &str,
    ) -> Result<cpt_core::Cpt, KernelError> {
        let cpt = import::parse_cpt(content, source_file)?;
        self.insert_rejecting_duplicates(GeotechnicalObject::Cpt(cpt.clone()))?;
        Ok(cpt)
    }

    fn insert_rejecting_duplicates(
        &mut self,
        object: GeotechnicalObject,
    ) -> Result<GeotechnicalObject, KernelError> {
        let id = object.id().to_owned();
        if self.objects.contains_key(&id) {
            return Err(KernelError::DuplicateObject { id });
        }
        self.objects.insert(id, object.clone());
        Ok(object)
    }

    /// Removes and returns an object by identifier.
    pub fn remove(&mut self, id: &str) -> Result<GeotechnicalObject, KernelError> {
        self.objects
            .remove(id)
            .ok_or_else(|| KernelError::ObjectNotFound { id: id.to_owned() })
    }

    /// Returns an object by identifier.
    pub fn get(&self, id: &str) -> Result<&GeotechnicalObject, KernelError> {
        self.objects
            .get(id)
            .ok_or_else(|| KernelError::ObjectNotFound { id: id.to_owned() })
    }

    /// Iterates over objects in ascending identifier order.
    pub fn objects(&self) -> impl Iterator<Item = &GeotechnicalObject> {
        self.objects.values()
    }

    /// Iterates over CPT objects in ascending object-identifier order.
    pub fn cpts(&self) -> impl Iterator<Item = &cpt_core::Cpt> {
        self.objects.values().filter_map(|object| match object {
            GeotechnicalObject::Cpt(cpt) => Some(cpt),
            GeotechnicalObject::BhrGt(_) | GeotechnicalObject::BhrG(_) => None,
        })
    }

    /// Returns project metadata.
    pub fn metadata(&self) -> &ProjectMetadata {
        &self.metadata
    }

    /// Replaces project metadata.
    pub fn set_metadata(&mut self, metadata: ProjectMetadata) {
        self.metadata = metadata;
    }

    /// Merges another project's objects according to an explicit duplicate policy.
    pub fn merge_from(
        &mut self,
        other: GeotechnicalProject,
        policy: DuplicatePolicy,
    ) -> Result<(), KernelError> {
        if policy == DuplicatePolicy::Reject {
            if let Some(id) = other
                .objects
                .keys()
                .find(|id| self.objects.contains_key(*id))
            {
                return Err(KernelError::DuplicateObject { id: id.clone() });
            }
        }
        self.objects.extend(other.objects);
        Ok(())
    }

    /// Detects soil layers for a CPT object.
    pub fn detect_cpt_layers(&self, id: &str) -> Result<Vec<cpt_core::Layer>, KernelError> {
        match self.get(id)? {
            GeotechnicalObject::Cpt(cpt) => Ok(cpt_core::detect_layers(cpt)),
            GeotechnicalObject::BhrGt(_) | GeotechnicalObject::BhrG(_) => {
                Err(KernelError::Conversion {
                    message: format!("geotechnical object {id} is not a CPT"),
                })
            }
        }
    }
}
