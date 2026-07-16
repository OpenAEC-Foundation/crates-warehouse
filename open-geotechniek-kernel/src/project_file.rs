use crate::{
    project::compatibility_bore_id, GeotechnicalObject, GeotechnicalProject, KernelError,
    ProjectMetadata,
};

impl GeotechnicalProject {
    /// Loads an existing legacy `.ifcgis` or IFCX project from JSON text.
    pub fn load_project_text(text: &str) -> Result<Self, KernelError> {
        Self::load_project_file(cpt_core::ifcgis::load(text)?)
    }

    /// Loads a parsed project file while retaining its compatibility sections.
    pub fn load_project_file(file: cpt_core::ifcgis::ProjectFile) -> Result<Self, KernelError> {
        let metadata = metadata_from_file(&file);
        let mut project = Self::new(metadata);

        for cpt in &file.cpts {
            insert_loaded(&mut project, GeotechnicalObject::Cpt(cpt.clone()))?;
        }

        for bore in &file.bores {
            match typed_bore(bore) {
                Some(object) => insert_loaded(&mut project, object)?,
                None => {
                    if let Some(id) = compatibility_bore_id(bore) {
                        if project.objects.contains_key(id)
                            || !project.compatibility_bore_ids.insert(id.to_owned())
                        {
                            return Err(KernelError::DuplicateObject { id: id.to_owned() });
                        }
                    }
                    project.compatibility_bores.push(bore.clone());
                }
            }
        }

        project.project_template = Some(file);
        Ok(project)
    }

    /// Converts the project to the shared in-memory project-file contract.
    pub fn to_project_file(&self) -> Result<cpt_core::ifcgis::ProjectFile, KernelError> {
        let mut file = self
            .project_template
            .clone()
            .unwrap_or_else(|| empty_project_file(&self.metadata));

        file.project = project_info(&self.metadata, file.project.date);
        file.cpts = self.cpts().cloned().collect();
        file.bores = self.compatibility_bores.clone();
        file.bores
            .extend(self.objects().filter_map(typed_bore_json));
        Ok(file)
    }

    /// Serializes the project as IFCX JSON without performing filesystem I/O.
    pub fn to_project_text(&self) -> Result<String, KernelError> {
        cpt_core::ifcgis::to_ifcx_json(&self.to_project_file()?).map_err(KernelError::from)
    }
}

fn metadata_from_file(file: &cpt_core::ifcgis::ProjectFile) -> ProjectMetadata {
    ProjectMetadata {
        title: file.project.title.clone(),
        client: file.project.client.clone(),
        location: file.project.location.clone(),
        project_number: file.project.project_number.clone(),
        author: file.project.author.clone(),
        date: Some(file.project.date),
    }
}

fn project_info(
    metadata: &ProjectMetadata,
    fallback_date: chrono::NaiveDate,
) -> cpt_core::ifcgis::ProjectInfo {
    cpt_core::ifcgis::ProjectInfo {
        kind: "OpenGeoProject".to_owned(),
        title: metadata.title.clone(),
        client: metadata.client.clone(),
        location: metadata.location.clone(),
        project_number: metadata.project_number.clone(),
        author: metadata.author.clone(),
        date: metadata.date.unwrap_or(fallback_date),
    }
}

fn empty_project_file(metadata: &ProjectMetadata) -> cpt_core::ifcgis::ProjectFile {
    let fallback_date = chrono::Utc::now().date_naive();
    cpt_core::ifcgis::ProjectFile {
        header: cpt_core::ifcgis::Header::new("Open Geotechniek Studio"),
        project: project_info(metadata, fallback_date),
        cpts: Vec::new(),
        bores: Vec::new(),
        crs: cpt_core::ifcgis::Crs::default(),
        tekening: None,
        title_block: None,
        gis: None,
        deliverable: None,
        calculations: Vec::new(),
    }
}

fn insert_loaded(
    project: &mut GeotechnicalProject,
    object: GeotechnicalObject,
) -> Result<(), KernelError> {
    let id = object.id().to_owned();
    if project.compatibility_bore_ids.contains(&id)
        || project.objects.insert(id.clone(), object).is_some()
    {
        return Err(KernelError::DuplicateObject { id });
    }
    Ok(())
}

fn typed_bore(value: &serde_json::Value) -> Option<GeotechnicalObject> {
    let wrapper_id = compatibility_bore_id(value)?;
    let source = value
        .get("source_xml")
        .or_else(|| value.get("sourceXml"))
        .or_else(|| value.get("metadata")?.get("source_xml"))
        .or_else(|| value.get("metadata")?.get("sourceXml"))?
        .as_str()?;
    let options = bro_xml::ParseOptions {
        retain_source: true,
    };
    let object = match bro_xml::parse_with_options(source, options).ok()? {
        bro_xml::BroDocument::BhrGt(document) => Some(GeotechnicalObject::BhrGt(document)),
        bro_xml::BroDocument::BhrG(document) => Some(GeotechnicalObject::BhrG(document)),
        bro_xml::BroDocument::Cpt(_) => None,
    }?;
    (object.id() == wrapper_id).then_some(object)
}

fn typed_bore_json(object: &GeotechnicalObject) -> Option<serde_json::Value> {
    match object {
        GeotechnicalObject::Cpt(_) => None,
        GeotechnicalObject::BhrGt(document) => Some(serde_json::json!({
            "id": document.common.bro_id,
            "position": document.common.position,
            "final_depth": document.final_depth,
            "layers": document.intervals,
            "metadata": {
                "document_type": "bhr_gt",
                "common": document.common,
                "boring_procedure": document.boring_procedure,
                "description_procedure": document.description_procedure,
                "source_xml": document.source_xml,
            }
        })),
        GeotechnicalObject::BhrG(document) => Some(serde_json::json!({
            "id": document.common.bro_id,
            "position": document.common.position,
            "final_depth": document.final_depth,
            "layers": document.intervals,
            "metadata": {
                "document_type": "bhr_g",
                "common": document.common,
                "source_xml": document.source_xml,
            }
        })),
    }
}
