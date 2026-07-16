use open_geotechniek_kernel::{GeotechnicalProject, KernelError, ObjectKind, ProjectMetadata};

#[test]
fn loads_and_round_trips_existing_project_shape() {
    let source = include_str!("fixtures/legacy-project.ifcgis");
    let project = GeotechnicalProject::load_project_text(source).unwrap();
    assert_eq!(project.cpts().count(), 1);
    let serialized = project.to_project_text().unwrap();
    let reopened = GeotechnicalProject::load_project_text(&serialized).unwrap();
    assert_eq!(reopened.cpts().count(), 1);
    assert_eq!(reopened.metadata().title, project.metadata().title);
}

#[test]
fn preserves_full_fidelity_template_sections_and_opaque_bores() {
    let source = include_str!("fixtures/legacy-project.ifcgis");
    let loaded = cpt_core::ifcgis::load(source).unwrap();
    let opaque_bore = loaded.bores[0].clone();
    let project = GeotechnicalProject::load_project_file(loaded).unwrap();

    let serialized = project.to_project_file().unwrap();
    assert_eq!(serialized.bores, vec![opaque_bore]);
    assert_eq!(serialized.crs.epsg, 28992);
    assert_eq!(serialized.title_block.unwrap().drawing_number, "D-01");
    assert_eq!(serialized.gis.unwrap().layers[0].id, "base");
    assert_eq!(serialized.calculations[0].input["value"], 12);
}

#[test]
fn converts_retained_bhr_xml_and_serializes_the_existing_bore_shape() {
    let source = include_str!("fixtures/legacy-project.ifcgis");
    let mut file = cpt_core::ifcgis::load(source).unwrap();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<BHR_GT_O xmlns="http://www.broservices.nl/xsd/dsbhr-gt/2.1">
  <broId>BHR000000000007</broId>
  <finalDepthBoring>2.0</finalDepthBoring>
  <boring><boringProcedure>handboring</boringProcedure></boring>
  <descriptiveBoreholeLog><layer><upperBoundary>0.0</upperBoundary><lowerBoundary>2.0</lowerBoundary><geotechnicalSoilName>zand</geotechnicalSoilName></layer></descriptiveBoreholeLog>
</BHR_GT_O>"#;
    file.bores.push(serde_json::json!({
        "id": "BHR000000000007",
        "metadata": { "source_xml": xml }
    }));

    let project = GeotechnicalProject::load_project_file(file).unwrap();
    assert_eq!(
        project.get("BHR000000000007").unwrap().kind(),
        ObjectKind::BhrGt
    );

    let serialized = project.to_project_file().unwrap();
    let typed = serialized
        .bores
        .iter()
        .find(|bore| bore["id"] == "BHR000000000007")
        .unwrap();
    assert!(typed.get("position").is_some());
    assert_eq!(typed["final_depth"], 2.0);
    assert_eq!(typed["layers"].as_array().unwrap().len(), 1);
    assert_eq!(typed["metadata"]["source_xml"], xml);
}

#[test]
fn direct_bhr_gt_import_remains_typed_after_project_round_trip() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let xml = include_str!("fixtures/bhr-gt-minimal.xml");
    project.import_bro(xml, "bhr-gt.xml").unwrap();

    let text = project.to_project_text().unwrap();
    let reopened = GeotechnicalProject::load_project_text(&text).unwrap();
    let object = reopened.get("BHR000000000001").unwrap();
    assert_eq!(object.kind(), ObjectKind::BhrGt);
    match object {
        open_geotechniek_kernel::GeotechnicalObject::BhrGt(document) => {
            assert_eq!(document.source_xml.as_deref(), Some(xml));
        }
        _ => unreachable!(),
    }
}

#[test]
fn direct_bhr_g_import_remains_typed_after_project_round_trip() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let xml = include_str!("fixtures/bhr-g-minimal.xml");
    project.import_bro(xml, "bhr-g.xml").unwrap();

    let text = project.to_project_text().unwrap();
    let reopened = GeotechnicalProject::load_project_text(&text).unwrap();
    let object = reopened.get("BHR000000000002").unwrap();
    assert_eq!(object.kind(), ObjectKind::BhrG);
    match object {
        open_geotechniek_kernel::GeotechnicalObject::BhrG(document) => {
            assert_eq!(document.source_xml.as_deref(), Some(xml));
        }
        _ => unreachable!(),
    }
}

#[test]
fn rejects_a_loaded_cpt_that_collides_with_an_opaque_bore_id() {
    let source = include_str!("fixtures/legacy-project.ifcgis");
    let mut file = cpt_core::ifcgis::load(source).unwrap();
    let mut duplicate = file.cpts[0].clone();
    duplicate.id = "LEGACY-BORE-1".to_owned();
    file.cpts.push(duplicate);

    let error = GeotechnicalProject::load_project_file(file).unwrap_err();
    assert!(matches!(
        error,
        KernelError::DuplicateObject { ref id } if id == "LEGACY-BORE-1"
    ));
}

#[test]
fn rejects_import_colliding_with_opaque_bore_without_mutating_the_project() {
    let source = include_str!("fixtures/legacy-project.ifcgis");
    let mut project = GeotechnicalProject::load_project_text(source).unwrap();
    let before_objects = project.objects().count();
    let before_bores = project.to_project_file().unwrap().bores;
    let xml = r#"<BHR_GT_O xmlns="http://www.broservices.nl/xsd/dsbhr-gt/2.1">
  <broId>LEGACY-BORE-1</broId>
  <finalDepthBoring>1.0</finalDepthBoring>
  <boring />
</BHR_GT_O>"#;

    let error = project.import_bro(xml, "collision.xml").unwrap_err();
    assert!(matches!(
        error,
        KernelError::DuplicateObject { ref id } if id == "LEGACY-BORE-1"
    ));
    assert_eq!(project.objects().count(), before_objects);
    assert_eq!(project.to_project_file().unwrap().bores, before_bores);
}

#[test]
fn retained_xml_with_a_mismatched_wrapper_id_stays_opaque() {
    let source = include_str!("fixtures/legacy-project.ifcgis");
    let mut file = cpt_core::ifcgis::load(source).unwrap();
    let xml = include_str!("fixtures/bhr-gt-minimal.xml");
    let mismatched = serde_json::json!({
        "id": "WRAPPER-BHR-1",
        "metadata": { "source_xml": xml }
    });
    file.bores.push(mismatched.clone());

    let project = GeotechnicalProject::load_project_file(file).unwrap();
    assert!(matches!(
        project.get("BHR000000000001"),
        Err(KernelError::ObjectNotFound { .. })
    ));
    let serialized = project.to_project_file().unwrap();
    assert!(serialized.bores.contains(&mismatched));
}
